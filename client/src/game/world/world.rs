use crate::engine::render::manager::RenderManager;
use shared::world::{
    constants::{max_chunks_in_queue, CHUNK_PRIORITY_DISTANCE},
    data::{
        block::BlockInstance,
        chunk::{Chunk, ChunkData, ChunkState, CHUNK_SIZE},
    },
    generation::chunk_generator::ChunkGenerator,
};
use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{
    // engine::render::mesh::manager::RenderManager,
    game::{player::player::Player, render::meshing::world::WorldMesh},
};

#[derive(Clone)]
pub struct MeshSnapshot {
    pub main: Arc<Chunk>,
    pub neg_x: Option<Arc<Chunk>>,
    pub pos_x: Option<Arc<Chunk>>,
    pub neg_y: Option<Arc<Chunk>>,
    pub pos_y: Option<Arc<Chunk>>,
    pub neg_z: Option<Arc<Chunk>>,
    pub pos_z: Option<Arc<Chunk>>,
}

pub struct World {
    chunks: HashMap<(i32, i32, i32), ChunkData>,
    seed: u32,
    chunk_generator: ChunkGenerator,
    pending_validations: Vec<(i32, i32, i32, [u8; 2])>,
}

impl World {
    pub fn new(seed: u32) -> World {
        let max_chunks = max_chunks_in_queue() as usize;
        let chunk_generator = ChunkGenerator::with_max_pending(seed, max_chunks);

        return World {
            chunks: HashMap::new(),
            seed: seed,
            chunk_generator: chunk_generator,
            pending_validations: vec![],
        };
    }

    pub fn get_mesh_snapshot(&self, cx: i32, cy: i32, cz: i32) -> MeshSnapshot {
        MeshSnapshot {
            main: Arc::new(self.get_chunk(cx, cy, cz).cloned().unwrap()),
            neg_x: self.get_chunk(cx - 1, cy, cz).cloned().map(Arc::new),
            neg_y: self.get_chunk(cx, cy - 1, cz).cloned().map(Arc::new),
            neg_z: self.get_chunk(cx, cy, cz - 1).cloned().map(Arc::new),
            pos_x: self.get_chunk(cx + 1, cy, cz).cloned().map(Arc::new),
            pos_y: self.get_chunk(cx, cy + 1, cz).cloned().map(Arc::new),
            pos_z: self.get_chunk(cx, cy, cz + 1).cloned().map(Arc::new),
        }
    }

    #[inline(always)]
    pub fn get_chunk_data(&self, cx: i32, cy: i32, cz: i32) -> Option<&ChunkData> {
        return self.chunks.get(&(cx, cy, cz));
    }

    #[inline(always)]
    pub fn get_chunk(&self, cx: i32, cy: i32, cz: i32) -> Option<&Chunk> {
        return self.chunks.get(&(cx, cy, cz)).map(|d| &d.chunk);
    }

    #[inline(always)]
    pub fn get_chunk_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Option<&mut ChunkData> {
        return self.chunks.get_mut(&(cx, cy, cz));
    }

    pub fn set_chunk(&mut self, cx: i32, cy: i32, cz: i32, chunk: Chunk) {
        self.chunks.insert((cx, cy, cz), ChunkData::new(chunk));
    }

    pub fn update(&mut self, render_manager: &mut RenderManager, world_mesh: &mut WorldMesh, player: &Player) {
        if !player.pos.has_changed() {
            return;
        }

        let _world_update_start = Instant::now();

        let needed_simulation_keys: Vec<(i32, i32, i32)> = player.get_simulation_chunk_keys();

        let current_keys: Vec<_> = self.chunks.keys().cloned().collect();
        for key in current_keys {
            if !needed_simulation_keys.contains(&key) {
                self.chunks.remove(&key);
                if let Some(mesh) = world_mesh.meshes.remove(&key) {
                    if let Some(id) = mesh.id {
                        render_manager.mesh_manager.free_data(id);
                    }
                }
            }
        }

        // println!("Time to unload chunks: {:3}ms.", world_update_start.elapsed().as_millis());

        // Identifier les chunks manquants
        let missing_keys: Vec<_> = needed_simulation_keys
            .iter()
            .filter(|k| !self.chunks.contains_key(k))
            .cloned()
            .collect();

        let player_pos = player.get_pos();
        let player_cx = (player_pos.x / CHUNK_SIZE as f32).floor() as i32;
        let _player_cy = (player_pos.y / CHUNK_SIZE as f32).floor() as i32;
        let player_cz = (player_pos.z / CHUNK_SIZE as f32).floor() as i32;

        let priority_distance_sq = CHUNK_PRIORITY_DISTANCE.powi(2);

        let mut priority_chunks: Vec<(i32, i32, i32)> = Vec::new();
        let mut normal_chunks: Vec<(i32, i32, i32)> = Vec::new();

        for key in missing_keys {
            let wx = key.0 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 / 2.0;
            let wy = key.1 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 / 2.0;
            let wz = key.2 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 / 2.0;

            let dx = wx - player_pos.x;
            let dy = wy - player_pos.y;
            let dz = wz - player_pos.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq < priority_distance_sq {
                priority_chunks.push(key);
            } else {
                normal_chunks.push(key);
            }
        }

        let mut sorted_priority: Vec<_> = priority_chunks
            .into_iter()
            .map(|key| {
                let dx = key.0 - player_cx;
                let dz = key.2 - player_cz;
                let dist_2 = dx * dx + dz * dz;
                (key, dist_2)
            })
            .collect();
        sorted_priority.sort_by(|a, b| a.1.cmp(&b.1));

        let mut sorted_normal: Vec<_> = normal_chunks
            .into_iter()
            .map(|key| {
                let dx = key.0 - player_cx;
                let dz = key.2 - player_cz;
                let dist_2 = dx * dx + dz * dz;
                (key, dist_2)
            })
            .collect();
        sorted_normal.sort_by(|a, b| a.1.cmp(&b.1));

        for (key, _) in sorted_priority {
            let _ = self.chunk_generator.request(key.0, key.1, key.2);
        }
        for (key, _) in sorted_normal {
            let _ = self.chunk_generator.request(key.0, key.1, key.2);
        }

        while let Some(result) = self.chunk_generator.try_recv() {
            let (cx, cy, cz, chunk_with_checksum) = result.output;

            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(neighbor) = self.chunks.get_mut(&(cx + dx, cy + dy, cz + dz)) {
                            neighbor.is_dirty = true;
                        }
                    }
                }
            }

            let mut new_chunk_data = chunk_with_checksum.chunk_data;
            new_chunk_data.is_dirty = true;
            self.chunks.insert((cx, cy, cz), new_chunk_data);

            self.pending_validations.push((cx, cy, cz, chunk_with_checksum.checksum));
        }
    }

    pub fn get_player_rendered_chunks(&self, player: &Player) -> Vec<&Chunk> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = player.get_rendered_chunk_range();

        let mut chunks: Vec<&Chunk> = Vec::new();

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    if let Some(data) = self.get_chunk_data(x, y, z) {
                        chunks.push(&data.chunk);
                    }
                }
            }
        }

        return chunks;
    }

    pub fn get_dirty_chunks(&self) -> Vec<(i32, i32, i32)> {
        self.chunks.iter().filter(|(_, data)| data.is_dirty).map(|(key, _)| *key).collect()
    }

    pub fn mark_chunk_clean(&mut self, cx: i32, cy: i32, cz: i32) {
        if let Some(data) = self.chunks.get_mut(&(cx, cy, cz)) {
            data.is_dirty = false;
        }
    }

    pub fn get_block_from_xyz(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        let cx: i32 = x.div_euclid(CHUNK_SIZE);
        let cy: i32 = y.div_euclid(CHUNK_SIZE);
        let cz: i32 = z.div_euclid(CHUNK_SIZE);

        let cbx: i32 = x.rem_euclid(CHUNK_SIZE);
        let cby: i32 = y.rem_euclid(CHUNK_SIZE);
        let cbz: i32 = z.rem_euclid(CHUNK_SIZE);

        if let Some(data) = self.get_chunk_data(cx, cy, cz) {
            return data.chunk.get_block_from_xyz(cbx, cby, cbz);
        } else {
            return BlockInstance::air();
        }
    }

    pub fn get_local_block_from_xyz(&self, lx: i32, ly: i32, lz: i32, cx: i32, cy: i32, cz: i32) -> BlockInstance {
        if !(0..CHUNK_SIZE).contains(&lx) || !(0..CHUNK_SIZE).contains(&ly) || !(0..CHUNK_SIZE).contains(&lz) {
            return self.get_block_from_xyz(lx + cx * CHUNK_SIZE, ly + cy * CHUNK_SIZE, lz + cz * CHUNK_SIZE);
        }

        if let Some(data) = self.get_chunk_data(cx, cy, cz) {
            return data.chunk.get_block_from_xyz(lx, ly, lz);
        } else {
            return BlockInstance::air();
        }
    }

    pub fn are_all_neighbors_ready(&self, cx: i32, cy: i32, cz: i32) -> bool {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    if let Some(data) = self.chunks.get(&(cx + dx, cy + dy, cz + dz)) {
                        if data.state != ChunkState::Ready {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn take_pending_validations(&mut self) -> Vec<(i32, i32, i32, [u8; 2])> {
        std::mem::take(&mut self.pending_validations)
    }
}
