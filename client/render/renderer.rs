use bytemuck::cast_slice;
use cgmath::{dot, EuclideanSpace, Point3, Vector3};
use engine::{core::frame::GameFrameData, geometry::vertex::generate_cube, render::render::Renderer};
use game::{
    constants::{CHUNK_VECTOR, HORIZONTAL_RENDER_DISTANCE, VERTICAL_RENDER_DISTANCE},
    world::data::chunk::CHUNK_SIZE_F,
};
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
        const BASE_REGION_HEIGHT: f32 = (VERTICAL_RENDER_DISTANCE + 1) as f32;
        const BASE_REGION_WIDTH: f32 = (HORIZONTAL_RENDER_DISTANCE + 1) as f32;

        let cam_eye = *state.player.state.camera.eye();
        let cam_position_chunk_aligned = cam_eye.map(|coord| coord - coord % CHUNK_SIZE_F);
        let cam_aabb = AABB::new_sized(
            // 3. Pour chaque coin, calculer la direction en espace chunk
            cam_position_chunk_aligned,
            Vector3::new(BASE_REGION_WIDTH, BASE_REGION_HEIGHT, BASE_REGION_WIDTH) * CHUNK_SIZE_F,
        );
        let cam_eye = cam_eye.to_vec();
        let cam_forward = state.player.state.camera.forward();
        let cam_frustum = state.player.state.camera.get_frustum_planes();

        let chunks_to_render = state.player.get_rendered_chunk_keys_set();
        let meshes = &state.world_mesh.meshes;

        for key in state.world_mesh.chunk_meshes.iter() {
            if !chunks_to_render.contains(key) {
                continue;
            }

            let Some(Some(id)) = meshes.get(key).map(|mesh| mesh.id) else {
                continue;
            };

            let min = Point3::new((key.0) as f32, (key.1) as f32, (key.2) as f32) * CHUNK_SIZE_F;
            let chunk_aabb = AABB::new_from_corner_and_dir(min, CHUNK_VECTOR);

            if !chunk_aabb.overlaps(&cam_aabb) {
                continue;
            }

            let min = min.to_vec();
            let max = min + CHUNK_VECTOR;

            // First, we check simply if the chunk to render is behind the camera.
            if Self::is_chunk_behind_camera(&min, &max, &cam_forward, &cam_eye) {
                continue;
            }

            // Second, we check if the chunk is within the field of view of the camera.
            if !Self::is_chunk_in_camera_frustum(&min, &max, &cam_frustum) {
                continue;
            }

            // If any of the above is true, we do not render the chunk.
            // We do the frustum check lately because it is more expansive,
            // on top of this, the first check would already eliminate ~50% of the candidates.

            out.insert(id);
        }
    }

    #[inline(never)]
    fn is_chunk_behind_camera(
        min: &Vector3<f32>,
        max: &Vector3<f32>,
        cam_forward: &Vector3<f32>,
        cam_eye: &Vector3<f32>,
    ) -> bool {
        let extent = (max - min) * 0.5;
        let center = min + extent;

        let radius = extent.x * cam_forward.x.abs() + extent.y * cam_forward.y.abs() + extent.z * cam_forward.z.abs();

        let distance = dot(*cam_forward, center - *cam_eye);

        distance + radius < 0.0
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
