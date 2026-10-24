use noise::{NoiseFn, Seedable, SuperSimplex};
use project_core::parallel::{Parallelizable, QueueFull, WorkResult, WorkerPool};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::world::data::block::BlockManager;
use crate::world::data::chunk::{Chunk, ChunkData, CHUNK_SIZE};
use crate::world::generation::biome::BiomeRegistry;
use crate::world::generation::chunk::ChunkWithChecksum;
use crate::world::generation::ore_gen::{OreGenConfig, OreVeinConfig};
use crate::world::generation::profiler::*;

// + caves proches
// - caves éloignées
pub const CAVE_SCALE: f64 = 0.00625;
// + caves larges
// - caves étroites
pub const CAVE_THRESHOLD: f64 = 0.05125;
pub const CAVE_MIN_DEPTH: i32 = 0;

pub const BIOME_CLIMATE_SCALE: f64 = 0.0005;

pub const ORE_REJECTION_MARGIN: f32 = 0.35;
pub const ORE_CELL_SIZE: usize = 4;
pub const ORE_GRID_CELLS: usize = (CHUNK_SIZE as usize) / ORE_CELL_SIZE; // 8
pub const ORE_GRID_POINTS: usize = ORE_GRID_CELLS + 1; // 9

pub type OreCellMask = [[[u32; ORE_GRID_CELLS]; ORE_GRID_CELLS]; ORE_GRID_CELLS];
pub type OreSampleGrid = [[[f32; ORE_GRID_POINTS]; ORE_GRID_POINTS]; ORE_GRID_POINTS];

#[derive(Clone)]
pub struct ChunkGenContext {
    pub seed: u32,
    pub surface: Arc<SuperSimplex>,
    pub cave_1: Arc<SuperSimplex>,
    pub cave_2: Arc<SuperSimplex>,
    pub temperature_noise: Arc<SuperSimplex>,
    pub humidity_noise: Arc<SuperSimplex>,
    pub biome_registry: BiomeRegistry,
    pub block_manager: Arc<RwLock<BlockManager>>,
    pub ore_configs: Vec<OreVeinConfig>,
    pub ore_noises: Vec<Arc<SuperSimplex>>,
}

impl ChunkGenContext {
    pub fn new(seed: u32, block_manager: Arc<RwLock<BlockManager>>) -> Self {
        let (ore_configs, ore_noises) = Self::load_ores(seed);

        Self {
            seed,
            surface: Arc::new(SuperSimplex::default().set_seed(seed)),
            cave_1: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(1000))),
            cave_2: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(2000))),
            temperature_noise: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(5000))),
            humidity_noise: Arc::new(SuperSimplex::default().set_seed(seed.wrapping_add(6000))),
            biome_registry: BiomeRegistry::load(crate::assets::resolve_asset_path("biomes.json")),
            block_manager,
            ore_configs,
            ore_noises,
        }
    }

    fn load_ores(seed: u32) -> (Vec<OreVeinConfig>, Vec<Arc<SuperSimplex>>) {
        let config =
            OreGenConfig::load(crate::assets::resolve_asset_path("ores.json")).unwrap_or(OreGenConfig { ores: Vec::new() });
        let configs = config.ores;
        let noises: Vec<Arc<SuperSimplex>> = configs
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let noise_seed = seed.wrapping_add((i as u32).wrapping_mul(3000));
                Arc::new(SuperSimplex::default().set_seed(noise_seed))
            })
            .collect();
        (configs, noises)
    }

    pub fn get_ore_count(&self) -> usize {
        self.ore_configs.len()
    }

    pub fn get_ore_config(&self, index: usize) -> Option<&OreVeinConfig> {
        self.ore_configs.get(index)
    }

    pub fn should_place_ore(&self, index: usize, wx: f64, wy: f64, wz: f64) -> bool {
        let Some(config) = self.ore_configs.get(index) else {
            return false;
        };
        let Some(noise) = self.ore_noises.get(index) else {
            return false;
        };
        let nx = wx * config.noise_scale;
        let ny = wy * config.noise_scale;
        let nz = wz * config.noise_scale;
        let value = noise.get([nx, ny, nz]).abs();
        value > config.threshold
    }

    #[inline(always)]
    pub fn get_biome_index(&self, wx: f64, wz: f64) -> usize {
        let t = self
            .temperature_noise
            .get([wx * BIOME_CLIMATE_SCALE, wz * BIOME_CLIMATE_SCALE]);
        let h = self.humidity_noise.get([wx * BIOME_CLIMATE_SCALE, wz * BIOME_CLIMATE_SCALE]);
        self.biome_registry.get_biome_index(t, h)
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
        if cave1 >= CAVE_THRESHOLD {
            return false;
        }
        let cave2 = self.cave_2.get([nx, ny, nz]).abs();
        cave2 < CAVE_THRESHOLD
    }

    /// Échantillonne les bruits de grottes sur une grille grossière 9x9x9 (pas de 4 blocs)
    /// et interpole trilinéairement pour chaque colonne (x, z), générant les masques de bits des grottes.
    pub fn compute_cave_column_masks(
        &self,
        cwx: i32,
        cwy: i32,
        cwz: i32,
        terrain_heights: &[i32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)],
    ) -> [u32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)] {
        let mut grid1 = [[[0.0f32; 9]; 9]; 9];
        let mut grid2 = [[[0.0f32; 9]; 9]; 9];

        for gx in 0..=8 {
            let wx = (cwx + (gx as i32) * 4) as f64;
            let nx = wx * CAVE_SCALE;
            for gz in 0..=8 {
                let wz = (cwz + (gz as i32) * 4) as f64;
                let nz = wz * CAVE_SCALE;
                for gy in 0..=8 {
                    let wy = (cwy + (gy as i32) * 4) as f64;
                    let ny = wy * CAVE_SCALE;
                    grid1[gx][gz][gy] = self.cave_1.get([nx, ny, nz]) as f32;
                    grid2[gx][gz][gy] = self.cave_2.get([nx, ny, nz]) as f32;
                }
            }
        }

        let mut cave_bits = [0u32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];
        let threshold = CAVE_THRESHOLD as f32;

        for x in 0..CHUNK_SIZE {
            let gx = (x / 4) as usize;
            let dx = (x % 4) as usize;
            let u = dx as f32 * 0.25;

            for z in 0..CHUNK_SIZE {
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);
                let terrain_y = terrain_heights[col];
                if cwy >= terrain_y {
                    continue;
                }

                let gz = (z / 4) as usize;
                let dz = (z % 4) as usize;
                let v = dz as f32 * 0.25;

                let w00 = (1.0 - u) * (1.0 - v);
                let w10 = u * (1.0 - v);
                let w01 = (1.0 - u) * v;
                let w11 = u * v;

                let mut col1 = [0.0f32; 9];
                let mut col2 = [0.0f32; 9];
                for gy in 0..=8 {
                    col1[gy] = w00 * grid1[gx][gz][gy]
                        + w10 * grid1[gx + 1][gz][gy]
                        + w01 * grid1[gx][gz + 1][gy]
                        + w11 * grid1[gx + 1][gz + 1][gy];
                    col2[gy] = w00 * grid2[gx][gz][gy]
                        + w10 * grid2[gx + 1][gz][gy]
                        + w01 * grid2[gx][gz + 1][gy]
                        + w11 * grid2[gx + 1][gz + 1][gy];
                }

                let mut mask_col = 0u32;
                'gy_loop: for gy in 0..8 {
                    let v1_start = col1[gy];
                    let v1_step = (col1[gy + 1] - v1_start) * 0.25;
                    let v2_start = col2[gy];
                    let v2_step = (col2[gy + 1] - v2_start) * 0.25;

                    for dy in 0..4 {
                        let y = (gy * 4 + dy) as i32;
                        let wy = y + cwy;
                        if wy >= terrain_y {
                            break 'gy_loop;
                        }
                        let depth = terrain_y - wy;
                        if depth < CAVE_MIN_DEPTH {
                            continue;
                        }

                        let val1 = v1_start + (dy as f32) * v1_step;
                        if val1.abs() < threshold {
                            let val2 = v2_start + (dy as f32) * v2_step;
                            if val2.abs() < threshold {
                                mask_col |= 1 << y;
                            }
                        }
                    }
                }
                cave_bits[col] = mask_col;
            }
        }

        cave_bits
    }

    /// Échantillonne les bruits des minerais sur une grille 9x9x9 et produit pour chaque mini-cube 4x4x4
    /// un masque de bits indiquant quels minerais peuvent potentiellement apparaître (rejet mathématique).
    ///
    /// `max_terrain_y` borne l'échantillonnage vertical : les points de grille au-dessus du relief
    /// sont dans l'air et ne seront jamais placés, on peut les sauter.
    pub fn compute_ore_rejection_grid(
        &self,
        cwx: i32,
        cwy: i32,
        cwz: i32,
        active_ores_by_y: &[u32; CHUNK_SIZE as usize],
        max_terrain_y: i32,
    ) -> (OreCellMask, [OreSampleGrid; 4]) {
        let mut cell_mask: OreCellMask = [[[0u32; ORE_GRID_CELLS]; ORE_GRID_CELLS]; ORE_GRID_CELLS];
        let mut sample_grids: [OreSampleGrid; 4] = [[[[0.0f32; ORE_GRID_POINTS]; ORE_GRID_POINTS]; ORE_GRID_POINTS]; 4];

        let mut chunk_active_ores = 0u32;
        for &mask in active_ores_by_y {
            chunk_active_ores |= mask;
        }

        if chunk_active_ores == 0 {
            return (cell_mask, sample_grids);
        }

        // Borne verticale globale : gy_max est le dernier point de grille utile.
        // Au-delà de max_terrain_y tout est air, pas besoin d'échantillonner.
        // On ajoute +1 car l'interpolation dans une cellule [gy..gy+1] nécessite le point gy+1.
        let gy_max_global = (((max_terrain_y - cwy) + ORE_CELL_SIZE as i32 - 1) / ORE_CELL_SIZE as i32)
            .clamp(0, ORE_GRID_POINTS as i32 - 1) as usize;

        let ore_count = self.get_ore_count().min(4);
        for i in 0..ore_count {
            if (chunk_active_ores & (1 << i)) == 0 {
                continue;
            }
            let Some(config) = self.ore_configs.get(i) else {
                continue;
            };
            let Some(noise) = self.ore_noises.get(i) else {
                continue;
            };
            let threshold = config.threshold as f32;
            let reject_threshold = (threshold - ORE_REJECTION_MARGIN).max(0.0);

            // Borne verticale par minerai : [depth_min, depth_max]
            let ore_gy_min = ((config.depth_min - cwy) / ORE_CELL_SIZE as i32).clamp(0, ORE_GRID_POINTS as i32 - 1) as usize;
            let ore_gy_max = (((config.depth_max - cwy) + ORE_CELL_SIZE as i32 - 1) / ORE_CELL_SIZE as i32)
                .clamp(0, ORE_GRID_POINTS as i32 - 1) as usize;
            // Le vrai plafond est le min entre le relief et la profondeur max du minerai
            let gy_top = ore_gy_max.min(gy_max_global);

            if ore_gy_min > gy_top {
                // Ce minerai n'a aucune couche utile dans ce chunk
                continue;
            }

            // 1. Échantillonner la grille pour le minerai i — uniquement les couches utiles
            for gx in 0..ORE_GRID_POINTS {
                let wx = (cwx + (gx as i32) * ORE_CELL_SIZE as i32) as f64;
                let nx = wx * config.noise_scale;
                for gz in 0..ORE_GRID_POINTS {
                    let wz = (cwz + (gz as i32) * ORE_CELL_SIZE as i32) as f64;
                    let nz = wz * config.noise_scale;
                    for gy in ore_gy_min..=gy_top {
                        let wy = (cwy + (gy as i32) * ORE_CELL_SIZE as i32) as f64;
                        let ny = wy * config.noise_scale;
                        sample_grids[i][gx][gz][gy] = noise.get([nx, ny, nz]) as f32;
                    }
                }
            }

            // 2. Déterminer les cellules 4x4x4 candidates — uniquement les cellules utiles
            let bit = 1u32 << i;
            let cell_gy_min = ore_gy_min; // cellule commence à ore_gy_min
            let cell_gy_max = if gy_top > 0 { gy_top - 1 } else { 0 }; // cellule [gy..gy+1], max gy = gy_top-1
            let cell_gy_max = cell_gy_max.min(ORE_GRID_CELLS - 1);

            if cell_gy_min <= cell_gy_max {
                for gx in 0..ORE_GRID_CELLS {
                    for gz in 0..ORE_GRID_CELLS {
                        for gy in cell_gy_min..=cell_gy_max {
                            let mut max_val = 0.0f32;
                            for cx in 0..=1 {
                                for cz in 0..=1 {
                                    for cy in 0..=1 {
                                        let v = sample_grids[i][gx + cx][gz + cz][gy + cy].abs();
                                        if v > max_val {
                                            max_val = v;
                                        }
                                    }
                                }
                            }
                            if max_val >= reject_threshold {
                                cell_mask[gx][gz][gy] |= bit;
                            }
                        }
                    }
                }
            }
        }

        (cell_mask, sample_grids)
    }

    /// VARIANTE 1 (EXACTE) : Évalue la formule exacte de placement du minerai.
    /// Avec le rejet préalable par cellule, cette fonction n'est appelée que sur les rares voxels candidats (~5%).
    #[inline(always)]
    pub fn should_place_ore_exact(&self, index: usize, wx: f64, wy: f64, wz: f64) -> bool {
        self.should_place_ore(index, wx, wy, wz)
    }

    /// VARIANTE 2 (APPROXIMÉE) : Interpole trilinéairement la valeur du minerai depuis la grille d'échantillons.
    /// Évite totalement le calcul de bruit 3D Simplex en boucle interne.
    #[inline(always)]
    pub fn should_place_ore_approx(&self, index: usize, x: i32, y: i32, z: i32, grid: &OreSampleGrid) -> bool {
        let Some(config) = self.ore_configs.get(index) else {
            return false;
        };

        let gx = (x / 4) as usize;
        let dx = (x % 4) as usize;
        let u = dx as f32 * 0.25;

        let gz = (z / 4) as usize;
        let dz = (z % 4) as usize;
        let v = dz as f32 * 0.25;

        let gy = (y / 4) as usize;
        let dy = (y % 4) as usize;
        let w = dy as f32 * 0.25;

        let w00 = (1.0 - u) * (1.0 - v);
        let w10 = u * (1.0 - v);
        let w01 = (1.0 - u) * v;
        let w11 = u * v;

        let v0 =
            w00 * grid[gx][gz][gy] + w10 * grid[gx + 1][gz][gy] + w01 * grid[gx][gz + 1][gy] + w11 * grid[gx + 1][gz + 1][gy];

        let v1 = w00 * grid[gx][gz][gy + 1]
            + w10 * grid[gx + 1][gz][gy + 1]
            + w01 * grid[gx][gz + 1][gy + 1]
            + w11 * grid[gx + 1][gz + 1][gy + 1];

        let val = v0 + (v1 - v0) * w;
        val.abs() > (config.threshold as f32)
    }
}

pub struct ChunkGen;

impl Parallelizable for ChunkGen {
    type Input = (i32, i32, i32);
    type Output = (i32, i32, i32, ChunkWithChecksum);
    type Context = ChunkGenContext;

    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let start = Instant::now();
        let (cx, cy, cz) = input;
        let chunk = Chunk::generate_with_context(cx, cy, cz, ctx);

        let checksum = chunk.compute_checksum();
        let chunk_data = ChunkData::new(chunk);
        let nanos = start.elapsed().as_nanos();
        gen_record_total(nanos);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cave_block_short_circuit_equivalence() {
        let bm = Arc::new(RwLock::new(BlockManager::new()));
        let ctx = ChunkGenContext::new(12345, bm);

        for x in 0..16 {
            let wx = (x * 4) as f64;
            for z in 0..16 {
                let wz = (z * 4) as f64;
                for y in 0..16 {
                    let wy = (y * 4) as f64;
                    let depth = 5;

                    let nx = wx * CAVE_SCALE;
                    let ny = wy * CAVE_SCALE;
                    let nz = wz * CAVE_SCALE;
                    let c1 = ctx.cave_1.get([nx, ny, nz]).abs();
                    let c2 = ctx.cave_2.get([nx, ny, nz]).abs();
                    let naive = c1 < CAVE_THRESHOLD && c2 < CAVE_THRESHOLD;

                    let optimized = ctx.is_cave_block(wx, wy, wz, depth);
                    assert_eq!(optimized, naive, "Divergence at ({wx}, {wy}, {wz})");
                }
            }
        }
    }

    #[test]
    fn test_compute_cave_column_masks_interpolated_high_agreement() {
        let bm = Arc::new(RwLock::new(BlockManager::new()));
        let ctx = ChunkGenContext::new(42, bm);

        let cwx = 0;
        let cwy = 0;
        let cwz = 0;
        let terrain_heights = [32i32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];

        let interpolated_masks = ctx.compute_cave_column_masks(cwx, cwy, cwz, &terrain_heights);

        let mut matches = 0usize;
        let total_voxels = (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize);

        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);
                let mask = interpolated_masks[col];

                for y in 0..CHUNK_SIZE {
                    let wy = (y + cwy) as f64;
                    let exact = ctx.is_cave_block(wx, wy, wz, 10);
                    let interp = (mask & (1 << y)) != 0;
                    if exact == interp {
                        matches += 1;
                    }
                }
            }
        }

        let agreement_rate = (matches as f64) / (total_voxels as f64);
        assert!(
            agreement_rate > 0.97,
            "L'interpolation trilinéaire 1/4 blocs doit concorder à plus de 97% avec le bruit exact (obtenu: {:.2}%)",
            agreement_rate * 100.0
        );
    }

    #[test]
    fn test_ore_rejection_zero_false_negatives() {
        let bm = Arc::new(RwLock::new(BlockManager::new()));
        let ctx = ChunkGenContext::new(42, bm);

        let cwx = 0;
        let cwy = 0;
        let cwz = 0;
        let active_ores = [0b1111u32; CHUNK_SIZE as usize];
        let max_terrain_y = CHUNK_SIZE; // tout le chunk est potentiellement rocheux

        let (cell_mask, _sample_grids) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores, max_terrain_y);

        // Vérifie sur tous les 32 768 voxels du chunk que si should_place_ore est vrai,
        // la cellule correspondante dans cell_mask A TOUJOURS le bit actif (0 faux négatif).
        for ore_idx in 0..ctx.get_ore_count().min(4) {
            let bit = 1u32 << ore_idx;
            for x in 0..CHUNK_SIZE {
                let wx = (x + cwx) as f64;
                let gx = (x / 4) as usize;
                for z in 0..CHUNK_SIZE {
                    let wz = (z + cwz) as f64;
                    let gz = (z / 4) as usize;
                    for y in 0..CHUNK_SIZE {
                        let wy = (y + cwy) as f64;
                        let gy = (y / 4) as usize;

                        if ctx.should_place_ore(ore_idx, wx, wy, wz) {
                            let cell_has_bit = (cell_mask[gx][gz][gy] & bit) != 0;
                            assert!(
                                cell_has_bit,
                                "FAUX NÉGATIF détecté pour le minerai {} à ({}, {}, {}) !",
                                ore_idx, x, y, z
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_should_place_ore_approx_high_agreement() {
        let bm = Arc::new(RwLock::new(BlockManager::new()));
        let ctx = ChunkGenContext::new(42, bm);

        let cwx = 0;
        let cwy = 0;
        let cwz = 0;
        let active_ores = [0b1111u32; CHUNK_SIZE as usize];
        let max_terrain_y = CHUNK_SIZE;

        let (_cell_mask, sample_grids) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores, max_terrain_y);

        for ore_idx in 0..ctx.get_ore_count().min(4) {
            let mut matches = 0usize;
            let total = (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize);

            for x in 0..CHUNK_SIZE {
                let wx = (x + cwx) as f64;
                for z in 0..CHUNK_SIZE {
                    let wz = (z + cwz) as f64;
                    for y in 0..CHUNK_SIZE {
                        let wy = (y + cwy) as f64;

                        let exact = ctx.should_place_ore_exact(ore_idx, wx, wy, wz);
                        let approx = ctx.should_place_ore_approx(ore_idx, x, y, z, &sample_grids[ore_idx]);
                        if exact == approx {
                            matches += 1;
                        }
                    }
                }
            }

            let rate = (matches as f64) / (total as f64);
            assert!(
                rate > 0.98,
                "L'approximation pour le minerai {} doit concorder à plus de 98% (obtenu: {:.2}%)",
                ore_idx,
                rate * 100.0
            );
        }
    }

    #[test]
    fn test_ore_vertical_bounding_skips_air() {
        // Vérifie que le bornage vertical ne crée pas de faux négatifs :
        // avec max_terrain_y = 16, les minerais au-dessus de y=16 ne doivent pas être produits.
        let bm = Arc::new(RwLock::new(BlockManager::new()));
        let ctx = ChunkGenContext::new(42, bm);

        let cwx = 0;
        let cwy = 0;
        let cwz = 0;
        let active_ores = [0b1111u32; CHUNK_SIZE as usize];

        // Cas 1 : grille complète (tout le chunk est rocheux)
        let (full_mask, _) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores, CHUNK_SIZE);

        // Cas 2 : grille bornée (relief à y=16, soit la moitié du chunk)
        let bounded_terrain_y = 16;
        let (bounded_mask, _) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores, bounded_terrain_y);

        // Les cellules sous le relief doivent être identiques (pas de faux négatifs)
        let cell_y_max = (bounded_terrain_y / ORE_CELL_SIZE as i32) as usize;
        for gx in 0..ORE_GRID_CELLS {
            for gz in 0..ORE_GRID_CELLS {
                for gy in 0..cell_y_max {
                    assert_eq!(
                        full_mask[gx][gz][gy], bounded_mask[gx][gz][gy],
                        "Divergence dans la cellule ({}, {}, {}) entre grille complète et bornée",
                        gx, gz, gy
                    );
                }
            }
        }
    }
}
