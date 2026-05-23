use cgmath::Point3;
use satiscore::log_warn_server;
use satiscore::world::data::block::BlockData;
use satiscore::world::data::block::{BlockInstance, BlockManager};
use satiscore::world::data::chunk::{global_position_to_chunk_pos, CHUNK_SIZE};
use satiscore::world::generation::chunk::ChunkWithChecksum;
use satiscore::world::generation::chunk_generator::{generate_chunks_parallel_blocking, generate_chunks_sequential};
use satiscore::world::modified_chunk::ModifiedWorld;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use physics::collision_world::CollisionWorld;

pub struct WorldState {
    pub seed: u32,
    pub block_manager: Arc<RwLock<BlockManager>>,
    pub world_generated_chunks: HashMap<(i32, i32, i32), ChunkWithChecksum>,
    pub modifications: ModifiedWorld,
}

impl WorldState {
    pub fn new() -> Self {
        let block_manager = Arc::new(RwLock::new(BlockManager::new()));

        let mut instance = Self {
            seed: 0,
            block_manager,
            world_generated_chunks: HashMap::new(),
            modifications: ModifiedWorld::new(),
        };

        instance.init();

        instance
    }

    pub fn init(&mut self) {
        let mut block_manager = self.block_manager.write().unwrap();

        let blocks = [
            BlockData::new("air"),
            BlockData::new("stone"),
            BlockData::new("dirt"),
            BlockData::new("grass"),
        ];

        for block in blocks {
            block_manager.register(block);
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn get_seed(&self) -> u32 {
        self.seed
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u32) {
        let cx = x.div_euclid(CHUNK_SIZE);
        let cy = y.div_euclid(CHUNK_SIZE);
        let cz = z.div_euclid(CHUNK_SIZE);

        if self.world_generated_chunks.contains_key(&(cx, cy, cz)) {
            self.modifications.set_block_at(x, y, z, BlockInstance::new(block_id));
        } else {
            log_warn_server!("Tentative de modification d'un chunk non chargé : ({}, {}, {})", cx, cy, cz);
        }
    }

    pub fn get_block(&self, gx: i32, gy: i32, gz: i32) -> BlockInstance {
        if let Some(block) = self.modifications.get_block_at(gx, gy, gz) {
            return *block;
        }
        let ((cx, cy, cz), intra) = global_position_to_chunk_pos(gx, gy, gz);
        if let Some(wrapped) = self.world_generated_chunks.get(&(cx, cy, cz)) {
            return wrapped
                .chunk_data
                .chunk
                .get_block_from_xyz(intra.x as i32, intra.y as i32, intra.z as i32);
        }
        BlockInstance::air()
    }

    /// Génère les chunks manquants à partir d'une liste de coordonnées.
    pub fn generate_missing(&mut self, coords: &[(i32, i32, i32)]) {
        let missing: Vec<_> = coords
            .iter()
            .filter(|c| !self.world_generated_chunks.contains_key(*c))
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        let generated = generate_chunks_parallel_blocking(Arc::clone(&self.block_manager), self.seed, missing);
        self.world_generated_chunks.extend(generated);
    }

    pub fn retain_chunks(&mut self, keep: &std::collections::HashSet<(i32, i32, i32)>) {
        self.world_generated_chunks.retain(|key, _| keep.contains(key));
        self.modifications.retain_chunks(keep);
    }

    pub fn get_required_chunks(cx: i32, cy: i32, cz: i32) -> std::collections::HashSet<(i32, i32, i32)> {
        let mut chunks = std::collections::HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.insert((cx + dx, cy + dy, cz + dz));
                }
            }
        }
        chunks
    }
    /// /!\ Fonction UNIQUEMENT Itérative et bloquante
    pub fn generate_chunks_between_2_pos(&mut self, p1: Point3<i32>, p2: Point3<i32>) -> HashMap<(i32, i32, i32), ChunkWithChecksum> {
        let min_x = p1.x.min(p2.x);
        let max_x = p1.x.max(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_y = p1.y.max(p2.y);
        let min_z = p1.z.min(p2.z);
        let max_z = p1.z.max(p2.z);

        let mut coords = Vec::new();
        for cx in min_x..=max_x {
            for cy in min_y..=max_y {
                for cz in min_z..=max_z {
                    coords.push((cx, cy, cz));
                }
            }
        }

        generate_chunks_sequential(Arc::clone(&self.block_manager), self.seed, coords)
    }
    /// /!\ Fonction UNIQUEMENT Itérative et bloquante
    pub fn generate_chunks_in_radius(&mut self, center: Point3<i32>, radius: i32) -> HashMap<(i32, i32, i32), ChunkWithChecksum> {
        let radius_sq = (radius as i64) * (radius as i64);
        let mut coords = Vec::new();
        for cx in (center.x - radius)..=(center.x + radius) {
            for cy in (center.y - radius)..=(center.y + radius) {
                for cz in (center.z - radius)..=(center.z + radius) {
                    let dx = (cx as i64 - center.x as i64).pow(2);
                    let dy = (cy as i64 - center.y as i64).pow(2);
                    let dz = (cz as i64 - center.z as i64).pow(2);
                    if dx + dy + dz <= radius_sq {
                        coords.push((cx, cy, cz));
                    }
                }
            }
        }

        generate_chunks_sequential(Arc::clone(&self.block_manager), self.seed, coords)
    }
}

impl CollisionWorld for WorldState {
    fn is_empty(&self) -> bool {
        self.world_generated_chunks.is_empty()
    }

    fn is_block_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_block(x, y, z).is_solid()
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}
