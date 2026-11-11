use std::{collections::HashSet, ops::ControlFlow};

use bytemuck::cast_slice;
use engine::{core::frame::GameFrameData, geometry::vertex::generate_cube, render::Renderer};
use game::{
    constants::CHUNK_VECTOR,
    types::Position,
    world::{chunk_position::ChunkPosition, data::chunk::CHUNK_SIZE},
};
use physics::aabb::AABB;
use project_core::math::geometry::{plane::Plane, vec3::Vec3};
use rustc_hash::FxBuildHasher;

use crate::{
    game::GameState,
    player::Player,
    render::meshing::{chunk::ChunkMesh, world::WorldMesh},
};

pub struct GameRenderer;

impl GameRenderer {
    pub fn render(state: &mut GameState, data: &mut GameFrameData, renderer: &mut Renderer) {
        // ATTENTION: dans le futur, trouver une alternative pour mieux mettre en cache les meshs ids
        // Peut créer des bugs de rendus difficilement débuggables
        if state.player.state.camera.view_proj().has_changed() {
            Self::update_chunks(&state.world_mesh, &state.player, data);
        };
        Self::update_players(state, data, renderer);
    }

    #[inline(never)]
    fn update_chunks(world_mesh: &WorldMesh, player: &Player, data: &mut GameFrameData) {
        let cam = &player.state.camera;
        let cam_frustum = cam.get_frustum_planes();
        let cam_frustum_aabb = cam.get_frustum_aabb();

        let (cam_x, cam_y, cam_z) = player.state.camera.eye().into();
        let view_proj = cam.view_proj().current();

        let meshes = &world_mesh.meshes;
        let render_range = player.get_rendered_chunk_range();

        let out = &mut data.visible_meshes;
        let aabb = &cam_frustum_aabb;

        data.camera.update(cam_x, cam_y, cam_z, (*view_proj).into());
        out.clear();

        for (coords, mesh) in meshes.iter() {
            if let ControlFlow::Break(_) = Self::cull_chunk(out, aabb, cam_frustum, &render_range, coords, mesh) {
                continue;
            }
        }
    }

    #[inline(never)]
    fn cull_chunk(
        out: &mut HashSet<u32, FxBuildHasher>,
        camera_frustum_aabb: &AABB,
        camera_frustum: &[Plane; 6],
        render_range: &[i32; 6],
        coords: &ChunkPosition,
        mesh: &ChunkMesh,
    ) -> ControlFlow<()> {
        let Some(id) = mesh.id else {
            return ControlFlow::Break(());
        };

        let (x, y, z) = coords.into();
        let corner = Position::new((x * CHUNK_SIZE) as f32, (y * CHUNK_SIZE) as f32, (z * CHUNK_SIZE) as f32);
        let chunk_aabb = AABB::from_corner_and_extent(corner, CHUNK_VECTOR);

        // AABB pre-check : ~80% of chunks are eliminated in 11 simple operations: 3 LEQ, 3 GEQ and 5 ANDs.
        if !chunk_aabb.intersects(camera_frustum_aabb) {
            return ControlFlow::Break(());
        }

        // Check to avoid edge cases & not-to-render chunks (in the future, e.g.: occluded chunks).
        // As efficient as the AABB pre-check.
        if !Self::is_chunk_in_render_range(&render_range, coords) {
            return ControlFlow::Break(());
        }

        // Last frustum check: we eliminate chunks whose AABB is outside the frustum of the camera (not visible).
        // Did in last because it's the most expansive check (scalar products).
        if !Self::is_chunk_in_camera_frustum(&chunk_aabb, camera_frustum) {
            return ControlFlow::Break(());
        }

        out.insert(id);

        ControlFlow::Continue(())
    }

    /// Indique si le chunk appartient à la boîte de rendu autour du joueur.
    ///
    /// `render_range` = [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz].
    #[inline(always)]
    fn is_chunk_in_render_range(render_range: &[i32; 6], coords: &ChunkPosition) -> bool {
        let (x, y, z) = coords.into();
        x >= render_range[0]
            && x <= render_range[1]
            && y >= render_range[2]
            && y <= render_range[3]
            && z >= render_range[4]
            && z <= render_range[5]
    }

    #[inline(never)]
    fn is_chunk_in_camera_frustum(chunk_aabb: &AABB, camera_frustum: &[Plane; 6]) -> bool {
        let min = chunk_aabb.min;
        let max = chunk_aabb.max;
        for p in camera_frustum {
            let positive = Vec3::new(
                if *p.normal.x() >= 0.0 { *max.x() } else { *min.x() },
                if *p.normal.y() >= 0.0 { *max.y() } else { *min.y() },
                if *p.normal.z() >= 0.0 { *max.z() } else { *min.z() },
            );
            if p.distance(&positive) < 0.0 {
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
                let player_data = generate_cube(new_pos);
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
