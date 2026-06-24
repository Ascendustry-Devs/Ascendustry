use std::{collections::HashSet, ops::ControlFlow};

use bytemuck::cast_slice;
use cgmath::{Matrix4, Vector3};
use engine::{core::frame::GameFrameData, geometry::vertex::generate_cube, render::render::Renderer};
use game::{constants::CHUNK_VECTOR, world::data::chunk::CHUNK_SIZE_F};
use physics::aabb::AABB;
use project_core::geometry::plane::Plane;
use rustc_hash::FxBuildHasher;

use crate::{game::GameState, render::meshing::chunk::ChunkMesh};

pub struct GameRenderer;

impl GameRenderer {
    pub fn render(state: &mut GameState, data: &mut GameFrameData, renderer: &mut Renderer) {
        // ATTENTION: dans le futur, trouver une alternative pour mieux mettre en cache les meshs ids
        // Peut créer des bugs de rendus difficilement débuggables
        if let Some(view_proj) = state.player.state.camera.view_proj().change() {
            Self::update_chunks(state, data, renderer, &view_proj.clone());
        };
        Self::update_players(state, data, renderer);
    }

    #[inline(never)]
    fn update_chunks(state: &mut GameState, data: &mut GameFrameData, renderer: &Renderer, view_proj: &Matrix4<f32>) {
        state.player.state.camera.aspect.update(renderer.render_options.aspect);

        let (cam_x, cam_y, cam_z) = {
            let pos = state.player.state.camera.eye();
            (pos.x, pos.y, pos.z)
        };

        data.camera.update(cam_x, cam_y, cam_z, (*view_proj).into());
        data.visible_meshes.clear();

        let cam = &state.player.state.camera;
        let cam_frustum = cam.get_frustum_planes();
        let cam_frustum_aabb = cam.get_frustum_aabb();

        let meshes = &state.world_mesh.meshes;
        let chunks_to_render = state.player.get_rendered_chunk_keys_set();

        let out = &mut data.visible_meshes;
        let aabb = &cam_frustum_aabb;

        for (coords, mesh) in meshes.iter() {
            if let ControlFlow::Break(_) = Self::cull_chunk(out, aabb, cam_frustum, chunks_to_render, coords, mesh) {
                continue;
            }
        }
    }

    #[inline(never)]
    fn cull_chunk(
        out: &mut HashSet<u32, FxBuildHasher>,
        cam_frustum_aabb: &AABB,
        cam_frustum: &[Plane; 6],
        chunks_to_render: &HashSet<(i32, i32, i32), FxBuildHasher>,
        coords: &(i32, i32, i32),
        mesh: &ChunkMesh,
    ) -> ControlFlow<()> {
        let (x, y, z) = *coords;
        let min = Vector3::new(x as f32, y as f32, z as f32) * CHUNK_SIZE_F;
        let max = min + CHUNK_VECTOR;

        // AABB pre-check : ~80% of chunks are eliminated in 6 simple checks.
        if Self::is_chunk_outside_aabb(&min, &max, cam_frustum_aabb) {
            return ControlFlow::Break(());
        }

        // Frustum check: we eliminate chunks whose AABB is outside the frustum of the camera (not visible).
        if !Self::is_chunk_in_camera_frustum(&min, &max, cam_frustum) {
            return ControlFlow::Break(());
        }

        // Last check to avoid edge cases & not-to-render chunks (in the future, e.g.: occluded chunks).
        // Hash lookup is the slowest part of the hot loop.
        if !chunks_to_render.contains(coords) {
            return ControlFlow::Break(());
        }

        let Some(id) = mesh.id else {
            return ControlFlow::Break(());
        };

        out.insert(id);

        ControlFlow::Continue(())
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

    // Update renderer with remote player positions
    #[inline(never)]
    fn update_players(state: &mut GameState, data: &mut GameFrameData, renderer: &Renderer) {
        let mut alloc = renderer.render_manager.world_buffer.write().unwrap();
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
}
