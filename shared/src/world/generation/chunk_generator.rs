use crate::parallel::{Parallelizable, QueueFull, WorkResult, WorkerPool};
use crate::world::data::block::BlockManager;
use crate::world::data::chunk::{Chunk, ChunkData};
use crate::world::generation::chunk::ChunkWithChecksum;
use noise::{NoiseFn, Perlin, Seedable};
use std::sync::Arc;

pub const CAVE_SCALE: f64 = 0.025;
pub const CAVE_THRESHOLD: f64 = 0.04;
pub const CAVE_MIN_DEPTH: i32 = 0;

#[derive(Clone)]
pub struct ChunkGenContext {
    pub seed: u32,
    pub perlin: Arc<Perlin>,
    pub cave_noise_1: Arc<Perlin>,
    pub cave_noise_2: Arc<Perlin>,
    pub block_manager: Arc<BlockManager>,
}

impl ChunkGenContext {
    pub fn new(seed: u32, block_manager: Arc<BlockManager>) -> Self {
        Self {
            seed,
            perlin: Arc::new(Perlin::default().set_seed(seed)),
            cave_noise_1: Arc::new(Perlin::default().set_seed(seed.wrapping_add(1000))),
            cave_noise_2: Arc::new(Perlin::default().set_seed(seed.wrapping_add(2000))),
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
        let cave1 = self.cave_noise_1.get([nx, ny, nz]).abs();
        let cave2 = self.cave_noise_2.get([nx, ny, nz]).abs();
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
    pub fn new(block_manager: Arc<BlockManager>, seed: u32) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        let worker_count = num_cpus::get();
        Self {
            inner: WorkerPool::new(worker_count, ctx),
        }
    }

    pub fn with_max_pending(block_manager: Arc<BlockManager>, seed: u32, max_pending: usize) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        let worker_count = num_cpus::get();
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
}

/// Génère des chunks de manière séquentielle (sans parallélisme).
/// À utiliser UNIQUEMNT lorsque le nombre de chunks est faible (ex: 27 chunks server).
pub fn generate_chunks_sequential(
    block_manager: Arc<BlockManager>,
    seed: u32,
    coords: Vec<(i32, i32, i32)>,
) -> std::collections::HashMap<(i32, i32, i32), ChunkWithChecksum> {
    use std::collections::HashMap;

    let mut result_map = HashMap::new();
    let ctx = ChunkGenContext::new(seed, block_manager);

    for (cx, cy, cz) in coords {
        let chunk = Chunk::generate_with_context(cx, cy, cz, &ctx);
        let checksum = chunk.compute_checksum();
        let chunk_data = ChunkData::new(chunk);
        result_map.insert((cx, cy, cz), ChunkWithChecksum { chunk_data, checksum });
    }

    result_map
}
