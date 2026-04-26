use cgmath::Point3;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use shared::network::messages::{Position, Rotation};
use shared::world::data::chunk::Chunk;
use std::collections::HashMap;
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
pub struct CachedChunk {
    pub chunk: Chunk,
    pub checksum: [u8; 2],
}
impl CachedChunk {
    pub fn new(chunk: Chunk, checksum: [u8; 2]) -> CachedChunk {
        return CachedChunk {
            chunk: chunk,
            checksum: checksum,
        };
    }
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

    pub fn cache_chunk(&self, x: i32, y: i32, z: i32, chunk: Chunk) {
        let checksum = chunk.compute_checksum();
        self.inner.write().unwrap().cache_chunk(x, y, z, chunk, checksum);
    }

    pub fn cache_chunk_with_checksum(&self, x: i32, y: i32, z: i32, chunk: Chunk, checksum: [u8; 2]) {
        self.inner.write().unwrap().cache_chunk(x, y, z, chunk, checksum);
    }

    pub fn get_cached_chunk(&self, x: i32, y: i32, z: i32) -> Option<Chunk> {
        self.inner.read().unwrap().get_cached_chunk(x, y, z).map(|c| c.chunk.clone())
    }

    pub fn get_cached_checksum(&self, x: i32, y: i32, z: i32) -> Option<[u8; 2]> {
        self.inner.read().unwrap().get_cached_checksum(x, y, z)
    }
    pub fn generate_chunks_between_2_pos(&self, p1: Point3<i32>, p2: Point3<i32>) {
        self.inner.write().unwrap().generate_chunks_between_2_pos(p1, p2);
    }
    pub fn generate_chunks_in_radius(&self, p1: Point3<i32>, radius: i32) {
        self.inner.write().unwrap().generate_chunks_in_radius(p1, radius);
    }
}

pub struct GameStateInner {
    pub seed: u32,
    pub players: HashMap<u64, Player>,
    pub chunk_cache: HashMap<(i32, i32, i32), CachedChunk>,
}

impl Default for GameStateInner {
    fn default() -> Self {
        Self::new()
    }
}

impl GameStateInner {
    pub fn new() -> Self {
        Self {
            seed: 0,
            players: HashMap::new(),
            chunk_cache: HashMap::new(),
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
            player.position = position;
            player.rotation = rotation;
        }
    }

    pub fn cache_chunk(&mut self, x: i32, y: i32, z: i32, chunk: Chunk, checksum: [u8; 2]) {
        self.chunk_cache.insert((x, y, z), CachedChunk { chunk, checksum });
    }
    pub fn cache_cached_chunk(&mut self, x: i32, y: i32, z: i32, cached_chunk: CachedChunk) {
        self.chunk_cache.insert((x, y, z), cached_chunk);
    }

    pub fn is_chunk_already_generated(&mut self, x: i32, y: i32, z: i32) -> bool {
        match self.get_cached_chunk(x, y, z) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn generate_chunk_at(x: i32, y: i32, z: i32) -> CachedChunk {
        let chunk = Chunk::generate(x, y, z, GAME_STATE.get_seed());
        let checksum = chunk.compute_checksum();
        return CachedChunk::new(chunk, checksum);
    }

    // Square
    pub fn generate_chunks_between_2_pos(&mut self, p1: Point3<i32>, p2: Point3<i32>) {
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
                    if !self.is_chunk_already_generated(cx, cy, cz) {
                        coords.push((cx, cy, cz));
                    }
                }
            }
        }

        let results = crate::chunk_generator::generate_chunks_parallel(self.seed, coords);
        for ((cx, cy, cz), chunk_data) in results {
            self.cache_cached_chunk(
                cx,
                cy,
                cz,
                CachedChunk {
                    chunk: chunk_data.chunk_data.chunk,
                    checksum: chunk_data.checksum,
                },
            );
        }
    }

    pub fn generate_chunks_in_radius(&mut self, center: Point3<i32>, radius: i32) {
        let radius_sq = (radius as i64) * (radius as i64);

        let mut coords = Vec::new();
        for cx in (center.x - radius)..=(center.x + radius) {
            for cy in (center.y - radius)..=(center.y + radius) {
                for cz in (center.z - radius)..=(center.z + radius) {
                    let dx = (cx as i64 - center.x as i64).pow(2);
                    let dy = (cy as i64 - center.y as i64).pow(2);
                    let dz = (cz as i64 - center.z as i64).pow(2);
                    if dx + dy + dz <= radius_sq {
                        if !self.is_chunk_already_generated(cx, cy, cz) {
                            coords.push((cx, cy, cz));
                        }
                    }
                }
            }
        }

        let results = crate::chunk_generator::generate_chunks_parallel(self.seed, coords);
        for ((cx, cy, cz), chunk_data) in results {
            self.cache_cached_chunk(
                cx,
                cy,
                cz,
                CachedChunk {
                    chunk: chunk_data.chunk_data.chunk,
                    checksum: chunk_data.checksum,
                },
            );
        }
    }

    pub fn get_cached_chunk(&self, x: i32, y: i32, z: i32) -> Option<&CachedChunk> {
        self.chunk_cache.get(&(x, y, z))
    }

    pub fn get_cached_checksum(&self, x: i32, y: i32, z: i32) -> Option<[u8; 2]> {
        self.chunk_cache.get(&(x, y, z)).map(|c| c.checksum)
    }
}
