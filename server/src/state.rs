use cgmath::Point3;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use shared::network::messages::{Position, Rotation};
use shared::world::data::block::{BlockData, BlockManager};
use shared::world::data::chunk::CHUNK_SIZE_F;
use shared::world::generation::chunk::ChunkWithChecksum;
use shared::world::generation::chunk_generator::generate_chunks_sequential;
use shared::{log_server, time};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

lazy_static! {
    pub static ref GAME_STATE: GameState = GameState::new();
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub username: String,
    pub position: Position,
    pub rotation: Rotation,
}

#[derive(Clone)]
pub struct GameState {
    inner: Arc<RwLock<GameStateInner>>,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(GameStateInner::new())),
        }
    }

    pub fn get_block_manager(&self) -> Arc<BlockManager> {
        Arc::clone(&self.inner.read().unwrap().block_manager)
    }

    pub fn init_random_seed(&self) {
        let seed = rand::random::<u32>();
        self.inner.write().unwrap().set_seed(seed);
    }

    pub fn init_seed(&self, seed: u32) {
        self.inner.write().unwrap().set_seed(seed);
    }

    pub fn get_seed(&self) -> u32 {
        self.inner.read().unwrap().get_seed()
    }

    pub fn get_all_players_vec(&self) -> Option<Vec<Player>> {
        self.inner.read().unwrap().get_players_vec()
    }

    pub fn add_player(&self, id: u64, username: String) {
        self.inner.write().unwrap().add_player(id, username);
    }

    pub fn remove_player(&self, id: &u64) -> Option<Player> {
        self.inner.write().unwrap().remove_player(id)
    }

    pub fn update_player_position(&self, id: u64, position: Position, rotation: Rotation) {
        self.inner.write().unwrap().update_player_position(id, position, rotation);
    }

    pub fn generate_chunks_between_2_pos(&self, block_manager: Arc<BlockManager>, p1: Point3<i32>, p2: Point3<i32>) {
        self.inner.write().unwrap().generate_chunks_between_2_pos(block_manager, p1, p2);
    }

    pub fn generate_chunks_in_radius(&self, block_manager: Arc<BlockManager>, p1: Point3<i32>, radius: i32) {
        self.inner.write().unwrap().generate_chunks_in_radius(block_manager, p1, radius);
    }
}

pub struct GameStateInner {
    pub seed: u32,
    pub players: HashMap<u64, Player>,
    pub block_manager: Arc<BlockManager>,
    pub chunks: HashMap<(i32, i32, i32), ChunkWithChecksum>,
    pub player_chunks: HashMap<u64, HashSet<(i32, i32, i32)>>,
}

impl Default for GameStateInner {
    fn default() -> Self {
        Self::new()
    }
}

impl GameStateInner {
    pub fn new() -> Self {
        let block_manager = {
            let mut block_manager = BlockManager::new();

            let blocks = [
                BlockData::new("air"),
                BlockData::new("stone"),
                BlockData::new("dirt"),
                BlockData::new("grass"),
            ];

            for block in blocks {
                block_manager.register(block);
            }

            Arc::new(block_manager)
        };

        Self {
            seed: 0,
            players: HashMap::new(),
            block_manager,
            chunks: HashMap::new(),
            player_chunks: HashMap::new(),
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn get_seed(&self) -> u32 {
        self.seed
    }

    pub fn add_player(&mut self, id: u64, username: String) {
        let player = Player {
            id,
            username,
            position: Position { x: 0.0, y: 64.0, z: 0.0 },
            rotation: Rotation { x: 0.0, y: 0.0 },
        };
        self.players.insert(id, player);
    }

    pub fn remove_player(&mut self, id: &u64) -> Option<Player> {
        self.player_chunks.remove(id);
        self.cleanup_unused_chunks();
        self.players.remove(id)
    }

    pub fn get_player(&self, id: &u64) -> Option<&Player> {
        self.players.get(id)
    }

    pub fn get_players_vec(&self) -> Option<Vec<Player>> {
        Some(self.players.values().cloned().collect())
    }

    pub fn update_player_position(&mut self, id: u64, position: Position, rotation: Rotation) {
        if let Some(player) = self.players.get_mut(&id) {
            player.position = position.clone();
            player.rotation = rotation;
        }

        let (cx, cy, cz) = Self::position_to_chunk_pos(&position);
        let required_chunks = Self::get_required_chunks(cx, cy, cz);

        // log_server!("Joueur {}: position mise à jour, chunk ({}, {}, {}), {} chunks requis", id, cx, cy, cz, required_chunks.len());

        self.player_chunks.insert(id, required_chunks.clone());

        let missing_chunks: Vec<_> = required_chunks.iter().filter(|c| !self.chunks.contains_key(*c)).cloned().collect();

        // log_server!("Joueur {}: {} chunks manquants", id, missing_chunks.len());

        if !missing_chunks.is_empty() {
            log_server!("Génération de {} chunks...", missing_chunks.len());

            let generated = generate_chunks_sequential(Arc::clone(&self.block_manager), self.seed, missing_chunks.clone());
            log_server!("{} chunks générés", generated.len());
            for (key, _) in generated.iter() {
                log_server!("Chunk généré à {:?}", key);
            }
            self.chunks.extend(generated);
        }

        self.cleanup_unused_chunks();
    }

    fn position_to_chunk_pos(pos: &Position) -> (i32, i32, i32) {
        let chunk_x = (pos.x / CHUNK_SIZE_F).floor() as i32;
        let chunk_y = (pos.y / CHUNK_SIZE_F).floor() as i32;
        let chunk_z = (pos.z / CHUNK_SIZE_F).floor() as i32;
        (chunk_x, chunk_y, chunk_z)
    }

    fn get_required_chunks(cx: i32, cy: i32, cz: i32) -> HashSet<(i32, i32, i32)> {
        let mut chunks = HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.insert((cx + dx, cy + dy, cz + dz));
                }
            }
        }
        chunks
    }

    fn cleanup_unused_chunks(&mut self) {
        let all_required: HashSet<_> = self.player_chunks.values().flat_map(|chunks| chunks.iter()).cloned().collect();

        self.chunks.retain(|key, _| all_required.contains(key));
    }

    pub fn generate_chunks_between_2_pos(&mut self, block_manager: Arc<BlockManager>, p1: Point3<i32>, p2: Point3<i32>) {
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

        let _results = generate_chunks_sequential(block_manager, self.seed, coords);
    }

    pub fn generate_chunks_in_radius(&mut self, block_manager: Arc<BlockManager>, center: Point3<i32>, radius: i32) {
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

        let _results = generate_chunks_sequential(block_manager, self.seed, coords);
    }
}
