use noise::{NoiseFn, Seedable, SuperSimplex};
use project_core::parallel::{Parallelizable, QueueFull, WorkResult, WorkerPool};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::world::data::block::BlockManager;
use crate::world::data::chunk::{Chunk, ChunkData};
use crate::world::generation::chunk::ChunkWithChecksum;

// + caves proches
// - caves éloignées
pub const CAVE_SCALE: f64 = 0.0125;
// + caves larges
// - caves étroites
pub const CAVE_THRESHOLD: f64 = 0.15625;
pub const CAVE_MIN_DEPTH: i32 = 0;

#[derive(Clone)]
pub struct ChunkGenContext {
    pub seed: u32,
    pub surface: Arc<SuperSimplex>,
    pub cave_1: Arc<SuperSimplex>,
    pub cave_2: Arc<SuperSimplex>,
    pub block_manager: Arc<RwLock<BlockManager>>,
}

impl ChunkGenContext {
    pub fn new(seed: u32, block_manager: Arc<RwLock<BlockManager>>) -> Self {
        Self {
            seed,
            surface: Arc::new(SuperSimplex::default().set_seed(seed)),
            cave_1: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(1000))),
            cave_2: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(2000))),
            block_manager,
        }
    }

    #[inline(always)]
    pub fn is_cave_block(&self, wx: f64, wy: f64, wz: f64, depth: i32) -> bool {
        if depth < CAVE_MIN_DEPTH {
            return false;
        }
        let nx = wx * CAVE_SCALE;
        let ny = wy * CAVE_SCALE;
        let nz = wz * CAVE_SCALE;
        let cave1 = self.cave_1.get([nx, ny, nz]).abs();
        let cave2 = self.cave_2.get([nx, ny, nz]).abs();
        cave1 < CAVE_THRESHOLD && cave2 < CAVE_THRESHOLD
    }
}

pub struct ChunkGen;

impl Parallelizable for ChunkGen {
    type Input = (i32, i32, i32);
    type Output = (i32, i32, i32, ChunkWithChecksum);
    type Context = ChunkGenContext;

    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let (cx, cy, cz) = input;
        let chunk = Chunk::generate_with_context(cx, cy, cz, ctx);

        let checksum = chunk.compute_checksum();
        let chunk_data = ChunkData::new(chunk);
        (cx, cy, cz, ChunkWithChecksum { chunk_data, checksum })
    }
}

pub struct ChunkGenerator {
    inner: WorkerPool<ChunkGen>,
}

impl ChunkGenerator {
    pub fn new(block_manager: Arc<RwLock<BlockManager>>, seed: u32) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        let worker_count = num_cpus::get();
        Self {
            inner: WorkerPool::new(worker_count, ctx),
        }
    }

    pub fn with_max_pending(
        worker_count: usize,
        block_manager: Arc<RwLock<BlockManager>>,
        seed: u32,
        max_pending: usize,
    ) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        Self {
            inner: WorkerPool::with_max_pending(worker_count, ctx, Some(max_pending)),
        }
    }

    pub fn request(&self, cx: i32, cy: i32, cz: i32) -> Result<usize, QueueFull> {
        self.inner.submit((cx, cy, cz))
    }

    pub fn try_recv(&self) -> Option<WorkResult<(i32, i32, i32, ChunkWithChecksum)>> {
        self.inner.try_recv()
    }

    pub fn is_queue_full(&self) -> bool {
        self.inner.is_queue_full()
    }

    pub const fn dispose(&mut self) {}
}

pub fn generate_chunks_sequential(
    block_manager: Arc<RwLock<BlockManager>>,
    seed: u32,
    coords: Vec<(i32, i32, i32)>,
) -> FxHashMap<(i32, i32, i32), ChunkWithChecksum> {
    let mut result_map = HashMap::with_hasher(FxBuildHasher);
    let ctx = ChunkGenContext::new(seed, block_manager);

    for (cx, cy, cz) in coords {
        let chunk = Chunk::generate_with_context(cx, cy, cz, &ctx);
        let checksum = chunk.compute_checksum();
        let chunk_data = ChunkData::new(chunk);
        result_map.insert((cx, cy, cz), ChunkWithChecksum { chunk_data, checksum });
    }

    result_map
}

pub fn generate_chunks_parallel_blocking(
    block_manager: Arc<RwLock<BlockManager>>,
    seed: u32,
    coords: Vec<(i32, i32, i32)>,
) -> FxHashMap<(i32, i32, i32), ChunkWithChecksum> {
    let ctx = ChunkGenContext::new(seed, block_manager);

    coords
        .par_iter()
        .map(|(cx, cy, cz)| {
            let chunk = Chunk::generate_with_context(*cx, *cy, *cz, &ctx);
            let checksum = chunk.compute_checksum();
            let chunk_data = ChunkData::new(chunk);
            ((*cx, *cy, *cz), ChunkWithChecksum { chunk_data, checksum })
        })
        .collect()
}
