use game::assets::item_loader::load_item_definitions;
use game::assets::resolve_asset_path;
use game::inventory::ItemManager;
use game::world::chunk_position::ChunkPosition;
use game::world::data::block::{BlockInstance, BlockManager};
use game::world::data::chunk::{Chunk, CHUNK_SIZE_SQR_USIZE, CHUNK_SIZE_USIZE};
use game::world::generation::chunk::ChunkWithChecksum;
use game::world::generation::chunk_generator::{generate_chunks_parallel_blocking, generate_chunks_sequential, ChunkGenerator};
use game::world::local_position::LocalPosition;
use game::world::modified_chunk::ModifiedWorld;
use game::world::world_position::WorldPosition;
use game::world::{ChunkAccess, ChunkModification, ChunkStorage, World};
use network::messages::ChunkData;
use physics::collision_world::CollisionWorld;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

pub struct ServerWorld {
    seed: u32,
    chunks: ServerChunks,
    chunk_generator: ChunkGenerator,
    block_manager: Arc<RwLock<BlockManager>>,
    item_manager: Arc<RwLock<ItemManager>>,
    pub modifications: ModifiedWorld,
}

impl World for ServerWorld {
    type Storage = ServerChunks;

    fn seed(&self) -> u32 {
        self.seed
    }

    fn chunks(&self) -> &Self::Storage {
        &self.chunks
    }

    fn chunks_mut(&mut self) -> &mut Self::Storage {
        &mut self.chunks
    }

    fn chunk_generator(&self) -> &ChunkGenerator {
        &self.chunk_generator
    }

    fn block_manager(&self) -> &Arc<RwLock<BlockManager>> {
        &self.block_manager
    }

    fn item_manager(&self) -> &Arc<RwLock<ItemManager>> {
        &self.item_manager
    }
}

impl ServerWorld {
    pub fn new(seed: u32, blocks_path: impl AsRef<Path>, items_path: impl AsRef<Path>) -> Self {
        let block_manager = Arc::new(RwLock::new(BlockManager::new()));
        let item_manager = Arc::new(RwLock::new(ItemManager::new()));

        let mut instance = Self {
            seed,
            chunks: ServerChunks::new(),
            chunk_generator: ChunkGenerator::new(Arc::clone(&block_manager), seed),
            block_manager,
            item_manager,
            modifications: ModifiedWorld::new(),
        };

        instance.init(blocks_path, items_path);

        instance
    }

    pub fn init(&mut self, blocks_path: impl AsRef<Path>, items_path: impl AsRef<Path>) {
        {
            let mut item_manager = self.item_manager.write().unwrap();
            let items = load_item_definitions(items_path).expect("Failed to load item definitions");
            for item in items {
                item_manager.register(item);
            }
        }

        {
            let mut block_manager = self.block_manager.write().unwrap();
            let item_manager = self.item_manager.read().unwrap();

            block_manager
                .load_from_directory(blocks_path)
                .expect("Failed to load block definitions");

            block_manager.resolve_item_mappings(&item_manager);
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn get_seed(&self) -> u32 {
        self.seed
    }

    /// Génère les chunks manquants à partir d'une liste de coordonnées.
    pub fn generate_missing(&mut self, coords: &[ChunkPosition]) {
        let missing: Vec<_> = coords.iter().filter(|c| !self.chunks.contains_key(*c)).cloned().collect();
        if missing.is_empty() {
            return;
        }
        let generated = generate_chunks_parallel_blocking(Arc::clone(&self.block_manager), self.seed, missing);
        self.chunks.extend(generated.into_iter().map(|(position, generated)| {
            (
                position,
                ServerChunk {
                    chunk: (*generated.chunk_data.chunk).clone(),
                    checksum: Checksum::from(generated.checksum),
                },
            )
        }));
    }

    pub fn retain_chunks(&mut self, keep: &FxHashSet<ChunkPosition>) {
        self.chunks.retain(|key, _| keep.contains(key));
    }

    pub fn get_required_chunks(position: &ChunkPosition) -> FxHashSet<ChunkPosition> {
        let mut chunks = HashSet::with_hasher(FxBuildHasher);
        let (cx, cy, cz) = position.into();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.insert(ChunkPosition::new(cx + dx, cy + dy, cz + dz));
                }
            }
        }
        chunks
    }
    /// /!\ Fonction UNIQUEMENT Itérative et bloquante
    pub fn generate_chunks_between_2_pos(
        &mut self,
        a: &ChunkPosition,
        b: &ChunkPosition,
    ) -> FxHashMap<ChunkPosition, ChunkWithChecksum> {
        let (ax, ay, az) = a.into();
        let (bx, by, bz) = b.into();
        let min_x = ax.min(bx);
        let max_x = ax.max(bx);
        let min_y = ay.min(by);
        let max_y = ay.max(by);
        let min_z = az.min(bz);
        let max_z = az.max(bz);

        let mut coords = Vec::new();
        for cx in min_x..=max_x {
            for cy in min_y..=max_y {
                for cz in min_z..=max_z {
                    coords.push(ChunkPosition::new(cx, cy, cz));
                }
            }
        }

        generate_chunks_sequential(Arc::clone(&self.block_manager), self.seed, coords)
    }
    /// /!\ Fonction UNIQUEMENT Itérative et bloquante
    pub fn generate_chunks_in_radius(
        &mut self,
        center: &ChunkPosition,
        radius: i32,
    ) -> FxHashMap<ChunkPosition, ChunkWithChecksum> {
        let radius_sq = (radius as i64) * (radius as i64);
        let mut coords = Vec::new();
        for cx in (center.x() - radius)..=(center.x() + radius) {
            for cy in (center.y() - radius)..=(center.y() + radius) {
                for cz in (center.z() - radius)..=(center.z() + radius) {
                    let dx = (cx as i64 - center.x() as i64).pow(2);
                    let dy = (cy as i64 - center.y() as i64).pow(2);
                    let dz = (cz as i64 - center.z() as i64).pow(2);
                    if dx + dy + dz <= radius_sq {
                        coords.push(ChunkPosition::new(cx, cy, cz));
                    }
                }
            }
        }

        generate_chunks_sequential(Arc::clone(&self.block_manager), self.seed, coords)
    }
    pub fn collect_modified_chunks_data(&self) -> Vec<ChunkData> {
        self.modifications
            .chunks()
            .keys()
            .filter(|key| self.chunks.get(key).is_some())
            .map(|position| {
                let generated = self.chunks.get(position);
                let server_chunk = generated.expect("wtf? filter works but this not?");
                let mut blocks = server_chunk.get_chunk().blocks.clone();
                // Appliquer les modifications par-dessus
                if let Ok(modified) = self.modifications.get_chunk_at(position) {
                    for (coords, block) in modified.blocks() {
                        let idx = coords.x() as usize
                            + coords.y() as usize * CHUNK_SIZE_USIZE
                            + coords.z() as usize * CHUNK_SIZE_SQR_USIZE;
                        blocks[idx] = *block;
                    }
                }
                // Sérialiser avec bincode (déjà utilisé partout)
                let data = bincode::serialize(&blocks).unwrap();
                ChunkData {
                    position: *position,
                    data,
                }
            })
            .collect()
    }
}

impl CollisionWorld for ServerWorld {
    fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn is_block_solid(&self, position: &WorldPosition) -> bool {
        self.get_block(position).map_or(false, |block| block.is_solid())
    }
}

impl Default for ServerWorld {
    fn default() -> Self {
        Self::new(0, resolve_asset_path("blocks/"), resolve_asset_path("items/"))
    }
}

impl ChunkStorage for ServerChunks {
    type Value = ServerChunk;

    fn contains_key(&self, coords: &ChunkPosition) -> bool {
        self.inner.contains_key(coords)
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn extend<T: Iterator<Item = (ChunkPosition, Self::Value)>>(&mut self, iter: T) {
        self.inner.extend(iter);
    }

    fn retain<F: FnMut(&ChunkPosition, &mut Self::Value) -> bool>(&mut self, f: F) {
        self.inner.retain(f);
    }

    fn get(&self, position: &ChunkPosition) -> Option<&Self::Value> {
        self.inner.get(position)
    }

    fn get_mut(&mut self, position: &ChunkPosition) -> Option<&mut Self::Value> {
        self.inner.get_mut(position)
    }
}

impl ChunkAccess for ServerChunk {
    fn get_chunk(&self) -> &Chunk {
        &self.chunk
    }

    fn get_chunk_mut(&mut self) -> &mut Chunk {
        &mut self.chunk
    }
}

impl ChunkModification for ServerChunk {
    fn set_block(&mut self, position: &LocalPosition, block: BlockInstance) -> Option<BlockInstance> {
        self.chunk.set_block_from_local_xyz(position, block)
    }
}

impl ServerChunks {
    pub const fn new() -> Self {
        Self {
            inner: HashMap::with_hasher(FxBuildHasher),
        }
    }
}

#[derive(Default, Clone)]
pub struct ServerChunk {
    chunk: Chunk,
    checksum: Checksum,
}

#[derive(Default, Clone)]
pub struct Checksum {
    inner: [u8; 2],
}
impl From<[u8; 2]> for Checksum {
    fn from(checksum: [u8; 2]) -> Checksum {
        return Checksum { inner: checksum };
    }
}

pub struct ServerChunks {
    inner: FxHashMap<ChunkPosition, ServerChunk>,
}

impl ServerChunks {
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
