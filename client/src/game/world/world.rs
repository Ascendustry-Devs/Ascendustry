use crate::{
    engine::render::{
        manager::RenderManager,
        texture::{TextureArrayIndex, TextureManager},
    },
    game::api::texture_loader::{self, TextureLoader},
};
use shared::constants::{max_chunks_in_queue, CHUNK_PRIORITY_DISTANCE_SQR};
use shared::world::data::block::BlockData;
use shared::world::{
    data::{
        block::{BlockInstance, BlockManager},
        chunk::{Chunk, ChunkData, ChunkState, CHUNK_SIZE, CHUNK_SIZE_HALFED},
    },
    generation::chunk_generator::ChunkGenerator,
};
use std::{
    cmp::min,
    collections::{HashMap, VecDeque},
    i32,
    sync::{Arc, RwLock},
    time::Instant,
};

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
    block_manager: Arc<RwLock<BlockManager>>,
    dirty_queue: VecDeque<(i32, i32, i32)>,
}

impl World {
    pub fn new(seed: u32) -> World {
        let block_manager = Arc::new(RwLock::new(BlockManager::new()));

        // let max_chunks = max_chunks_in_queue() as usize;
        let worker_count = min((num_cpus::get() as f32 / 2.0).floor() as usize, 1);
        let chunk_generator =
            ChunkGenerator::with_max_pending(worker_count, Arc::clone(&block_manager), seed, max_chunks_in_queue() as usize);

        return World {
            chunks: HashMap::new(),
            seed: seed,
            chunk_generator: chunk_generator,
            block_manager: block_manager,
            dirty_queue: VecDeque::new(),
        };
    }

    pub fn init(&mut self, texture_loader: &mut TextureLoader) {
        let mut block_manager = self.block_manager.write().unwrap();

        let blocks = [
            (BlockData::new("air"), ""),
            (BlockData::new("stone"), "assets/images/stone.png"),
            (BlockData::new("dirt"), "assets/images/dirt.png"),
            (BlockData::new("grass"), "assets/images/grass.png"),
        ];

        for mut values in blocks {
            if let Ok(tex_id) = texture_loader.register(values.1.to_string(), TextureArrayIndex::Opaque) {
                values.0.texture_index = Some(tex_id.depth() as u32);
            };
            block_manager.register(values.0);
        }
    }

    pub fn get_mesh_snapshot(&self, cx: i32, cy: i32, cz: i32) -> MeshSnapshot {
        MeshSnapshot {
            main: Arc::clone(&self.chunks.get(&(cx, cy, cz)).unwrap().chunk),
            neg_x: self.chunks.get(&(cx - 1, cy, cz)).map(|d| Arc::clone(&d.chunk)),
            neg_y: self.chunks.get(&(cx, cy - 1, cz)).map(|d| Arc::clone(&d.chunk)),
            neg_z: self.chunks.get(&(cx, cy, cz - 1)).map(|d| Arc::clone(&d.chunk)),
            pos_x: self.chunks.get(&(cx + 1, cy, cz)).map(|d| Arc::clone(&d.chunk)),
            pos_y: self.chunks.get(&(cx, cy + 1, cz)).map(|d| Arc::clone(&d.chunk)),
            pos_z: self.chunks.get(&(cx, cy, cz + 1)).map(|d| Arc::clone(&d.chunk)),
        }
    }

    #[inline(always)]
    pub fn get_chunk_data(&self, cx: i32, cy: i32, cz: i32) -> Option<&ChunkData> {
        return self.chunks.get(&(cx, cy, cz));
    }

    #[inline(always)]
    pub fn get_chunk_data_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Option<&mut ChunkData> {
        return self.chunks.get_mut(&(cx, cy, cz));
    }

    #[inline(always)]
    pub fn get_chunk(&self, cx: i32, cy: i32, cz: i32) -> Option<&Chunk> {
        return self.chunks.get(&(cx, cy, cz)).map(|d| d.chunk.as_ref());
    }

    #[inline(always)]
    pub fn get_chunk_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Option<&mut ChunkData> {
        return self.chunks.get_mut(&(cx, cy, cz));
    }

    pub fn set_chunk(&mut self, cx: i32, cy: i32, cz: i32, chunk: Chunk) {
        self.chunks.insert((cx, cy, cz), ChunkData::new(chunk));
    }

    pub fn chunk_coords_from_block(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (x.div_euclid(CHUNK_SIZE), y.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
    }

    pub fn local_block_coords(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (x.rem_euclid(CHUNK_SIZE), y.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE))
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) -> bool {
        let (cx, cy, cz) = World::chunk_coords_from_block(x, y, z);
        let (lx, ly, lz) = World::local_block_coords(x, y, z);
        let Some(chunk) = self.get_chunk_data_mut(cx, cy, cz) else {
            return false;
        };
        println!("lx: {} ly: {} lz: {}", lx, ly, lz);
        let current_block = chunk.chunk.get_block_from_xyz(lx, ly, lz);
        if current_block == block {
            return false;
        } else {
            Arc::make_mut(&mut chunk.chunk).set_block_from_xyz(lx, ly, lz, block);
            return true;
        }
    }

    pub fn update(&mut self, render_manager: &mut RenderManager, world_mesh: &mut WorldMesh, player: &Player) {
        let _world_update_start = Instant::now();
        if player.state.cpos.has_changed() || self.chunks.is_empty() {
            let needed_simulation_keys: Vec<(i32, i32, i32)> = player.get_simulation_chunk_keys();

            let cpos = player.get_cpos().into();

            let radii_h_squared = (player.state.horizontal_render_distance * player.state.horizontal_render_distance) as i32;
            let radii_v_squared = (player.state.vertical_render_distance * player.state.vertical_render_distance) as i32;

            let keys_to_remove: Vec<_> = self
                .chunks
                .keys()
                .filter(|key| !is_chunk_in_range(*key, &cpos, radii_h_squared, radii_v_squared))
                .cloned()
                .collect();

            for k in keys_to_remove.iter() {
                self.chunks.remove(&k);
                if let Some(mesh) = world_mesh.meshes.remove(&k) {
                    if let Some(id) = mesh.id {
                        render_manager.mesh_manager.free_data(id);
                    }
                }
            }

            // Identifier les chunks manquants
            let missing_keys: Vec<_> = needed_simulation_keys
                .iter()
                .filter(|k| !self.chunks.contains_key(k))
                .cloned()
                .collect();

            let player_pos = player.get_pos();
            let player_cpos = player.get_cpos();

            let mut priority_chunks: Vec<(i32, i32, i32)> = Vec::new();
            let mut normal_chunks: Vec<(i32, i32, i32)> = Vec::new();

            for key in missing_keys {
                let wx = (key.0 * CHUNK_SIZE + CHUNK_SIZE_HALFED) as f32;
                let wy = (key.1 * CHUNK_SIZE + CHUNK_SIZE_HALFED) as f32;
                let wz = (key.2 * CHUNK_SIZE + CHUNK_SIZE_HALFED) as f32;

                let dx = wx - player_pos.x;
                let dy = wy - player_pos.y;
                let dz = wz - player_pos.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq < CHUNK_PRIORITY_DISTANCE_SQR {
                    priority_chunks.push(key);
                } else {
                    normal_chunks.push(key);
                }
            }

            let mut sorted_priority: Vec<_> = priority_chunks
                .into_iter()
                .map(|key| {
                    let dx = key.0 - player_cpos.x;
                    let dz = key.2 - player_cpos.z;
                    let dist_2 = dx * dx + dz * dz;
                    (key, dist_2)
                })
                .collect();
            sorted_priority.sort_by(|a, b| a.1.cmp(&b.1));

            let mut sorted_normal: Vec<_> = normal_chunks
                .into_iter()
                .map(|key| {
                    let dx = key.0 - player_cpos.x;
                    let dz = key.2 - player_cpos.z;
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
        }

        while let Some(result) = self.chunk_generator.try_recv() {
            let (cx, cy, cz, chunk_with_checksum) = result.output;

            const DIRECT_NEIGHBORS: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];

            let mut new_chunk_data = chunk_with_checksum.chunk_data;
            new_chunk_data.is_dirty = true;
            self.chunks.insert((cx, cy, cz), new_chunk_data);

            for (dx, dy, dz) in DIRECT_NEIGHBORS {
                if let Some(neighbor) = self.chunks.get_mut(&(cx + dx, cy + dy, cz + dz)) {
                    neighbor.is_dirty = true;
                }
            }
        }

        // let _world_update_end = _world_update_start.elapsed().as_millis();
        // if _world_update_end > 0 {
        //     println!("Time took for world update: {} ms", _world_update_end);
        // }
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
        // Only require that existing neighbors are Ready. If a neighbor chunk is missing,
        // treat it as not blocking mesh generation, to allow streaming without stalling.
        const DIRECT_NEIGHBORS: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
        for (dx, dy, dz) in DIRECT_NEIGHBORS {
            if let Some(neighbor) = self.chunks.get(&(cx + dx, cy + dy, cz + dz)) {
                if neighbor.state != ChunkState::Ready {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Retourne vrai si aucun chunk n'est chargé
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn dispose(&mut self) {
        self.chunks.clear();
        // TODO: faire fonctionner -> self.block_manager.dispose();
        self.chunk_generator.dispose();
    }
}

#[inline(always)]
fn is_chunk_in_range(c: &(i32, i32, i32), center: &(i32, i32, i32), radius_h_squared: i32, radius_v_squared: i32) -> bool {
    let dx = c.0 - center.0;
    let dy = c.1 - center.1;
    let dz = c.2 - center.2;

    (dx * dx) / radius_h_squared + (dy * dy) / radius_v_squared + (dz * dz) / radius_h_squared <= 1
}
