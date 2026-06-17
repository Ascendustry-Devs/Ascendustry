use bytemuck::cast_slice;
use cgmath::Vector3;
use engine::{core::frame::GameFrameData, geometry::vertex::generate_cube, render::render::Renderer};
use game::{constants::CHUNK_VECTOR, world::data::chunk::CHUNK_SIZE_F};
use physics::aabb::AABB;
use project_core::geometry::plane::Plane;
use rustc_hash::FxHashSet;

use crate::game::GameState;

pub struct GameRenderer;

impl GameRenderer {
    pub fn render(state: &mut GameState, data: &mut GameFrameData, renderer: &mut Renderer) {
        // ATTENTION: dans le futur, trouver une alternative pour mieux mettre en cache les meshs ids
        // Peut créer des bugs de rendus difficilement débuggables
        if let Some(view_proj) = state.player.state.camera.view_proj().change() {
            let (cam_x, cam_y, cam_z) = {
                let pos = state.player.state.camera.eye();
                (pos.x, pos.y, pos.z)
            };
            data.camera.update(cam_x, cam_y, cam_z, (*view_proj).into());

            state.player.state.camera.aspect.update(renderer.render_options.aspect);

            data.visible_meshes.clear();

            Self::cull_chunks(state, &mut data.visible_meshes);
        };
        // Update renderer with remote player positions
        let alloc = &mut renderer.render_manager.world_buffer.write().unwrap();
        for p in state.remote_players.get_all_mut().iter_mut() {
            if let Some(new_pos) = p.position.change() {
                let player_data = generate_cube(new_pos.0, new_pos.1, new_pos.2);
                let raw_data = cast_slice(&player_data);
                if let Some(mesh_id) = p.mesh_id {
                    if let Some(update_err) = alloc.update(mesh_id, raw_data).err() {
                        println!("Failed to update mesh id {}.\nError: {}", mesh_id, update_err);
                    };
                } else {
                    p.mesh_id = alloc.add(raw_data).ok();
                }
            }
            data.visible_meshes.replace(p.mesh_id.unwrap());
        }
    }

    #[inline(never)]
    fn cull_chunks(state: &GameState, out: &mut FxHashSet<u32>) {
        let cam = &state.player.state.camera;
        let cam_frustum_aabb = cam.get_frustum_aabb();
        let cam_frustum = cam.get_frustum_planes();

        let meshes = &state.world_mesh.meshes;
        let chunks_to_render = state.player.get_rendered_chunk_keys_set();

        for (coords, mesh) in meshes.iter() {
            let (x, y, z) = *coords;
            let min = Vector3::new(x as f32, y as f32, z as f32) * CHUNK_SIZE_F;
            let max = min + CHUNK_VECTOR;

            // AABB pre-check : ~80% of chunks are eliminated in 6 simple checks.
            if Self::is_chunk_outside_aabb(&min, &max, &cam_frustum_aabb) {
                continue;
            }

            // Frustum check: we eliminate chunks whose AABB is outside the frustum of the camera (not visible).
            if !Self::is_chunk_in_camera_frustum(&min, &max, &cam_frustum) {
                continue;
            }

            // Last check to avoid edge cases & not-to-render chunks (in the future, e.g.: occluded chunks).
            // Hash lookup is the slowest part of the hot loop.
            if !chunks_to_render.contains(coords) {
                continue;
            }

            let Some(id) = mesh.id else {
                continue;
            };

            out.insert(id);
        }
    }

    #[inline(never)]
    fn is_chunk_outside_aabb(min: &Vector3<f32>, max: &Vector3<f32>, aabb: &AABB) -> bool {
        min.x > aabb.max.x
            || max.x < aabb.min.x
            || min.y > aabb.max.y
            || max.y < aabb.min.y
            || min.z > aabb.max.z
            || max.z < aabb.min.z
    }

    #[inline(never)]
    fn is_chunk_in_camera_frustum(min: &Vector3<f32>, max: &Vector3<f32>, planes: &[Plane; 6]) -> bool {
        for p in planes {
            let positive = Vector3::new(
                if p.normal.x >= 0.0 { max.x } else { min.x },
                if p.normal.y >= 0.0 { max.y } else { min.y },
                if p.normal.z >= 0.0 { max.z } else { min.z },
            );
            if p.distance(positive) < 0.0 {
                return false;
            }
        }
        true
    }
}
