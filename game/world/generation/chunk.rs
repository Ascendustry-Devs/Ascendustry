#![allow(static_mut_refs)]
use std::sync::{Arc, RwLock};
use std::time::Instant;

use noise::NoiseFn;

use crate::world::data::block::{BlockInstance, BlockManager};
use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER, CHUNK_SIZE};
use crate::world::generation::chunk_generator::ChunkGenContext;
use crate::world::generation::profiler::{
    gen_record_section, GEN_HIST_CAVE, GEN_HIST_NOISE, GEN_HIST_ORE, GEN_HIST_OTHER, GEN_HIST_SURFACE, GEN_HIST_TERRAIN,
    GEN_HIST_VEGETATION, GEN_STAT_CAVE, GEN_STAT_NOISE, GEN_STAT_ORE, GEN_STAT_OTHER, GEN_STAT_SURFACE, GEN_STAT_TERRAIN,
    GEN_STAT_VEGETATION,
};

pub struct ChunkWithChecksum {
    pub chunk_data: crate::world::data::chunk::ChunkData,
    pub checksum: [u8; 2],
}

impl Chunk {
    #[inline]
    pub fn generate(block_manager: Arc<RwLock<BlockManager>>, cx: i32, cy: i32, cz: i32, seed: u32) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        Self::generate_with_context(cx, cy, cz, &ctx)
    }

    #[inline]
    pub fn generate_with_context(cx: i32, cy: i32, cz: i32, ctx: &ChunkGenContext) -> Self {
        let t0 = Instant::now();
        let cwx = cx * CHUNK_SIZE;
        let cwy = cy * CHUNK_SIZE;
        let cwz = cz * CHUNK_SIZE;

        let t_noise_start = Instant::now();
        let blocks = ctx.block_manager.read().unwrap();

        let block_id = |name: &str| -> u32 {
            blocks
                .get_block_by_string(String::from(name))
                .unwrap_or_else(|| panic!("Did not find block '{name}' in block manager"))
                .get_id()
        };

        let grass_id = block_id("grass");
        let dirt_id = block_id("dirt");
        let stone_id = block_id("stone");
        let sand_id = block_id("sand");
        let snow_id = block_id("snow");
        let water_id = block_id("water");

        let ore_count = ctx.get_ore_count().min(32);
        let mut ore_ids: [Option<u32>; 32] = [None; 32];
        for i in 0..ore_count {
            if let Some(config) = ctx.get_ore_config(i) {
                ore_ids[i] = blocks.get_block_by_string(config.block_id.clone()).map(|b| b.get_id());
            }
        }

        drop(blocks);

        let blocks_arr = vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER];
        let t_noise_end = t_noise_start.elapsed().as_nanos();

        #[derive(Clone, Copy)]
        struct ColInfo {
            terrain_y: i32,
            subsurface_depth: i32,
            surface_id: u32,
            subsurface_id: u32,
        }

        let mut col_info = [ColInfo {
            terrain_y: 0,
            subsurface_depth: 3,
            surface_id: grass_id,
            subsurface_id: dirt_id,
        }; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];

        let mut terrain_heights = [0i32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];
        let mut max_terrain_y = i32::MIN;
        let mut t_surface_acc: u128 = 0;
        let mut t_terrain_acc: u128 = 0;

        // Pré-calcul unique par colonne (biome + terrain + ids)
        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);

                let t_s = Instant::now();
                let biome_idx = ctx.get_biome_index(wx, wz);
                let biome = &ctx.biome_registry.biomes[biome_idx];
                t_surface_acc += t_s.elapsed().as_nanos();

                let t_t = Instant::now();
                let nx = wx * biome.terrain.scale;
                let nz = wz * biome.terrain.scale;
                let valeur = ctx.surface.get([nx, nz]);
                let terrain_y = valeur.mul_add(biome.terrain.amplitude, biome.terrain.base_height) as i32;
                t_terrain_acc += t_t.elapsed().as_nanos();

                terrain_heights[col] = terrain_y;
                if terrain_y > max_terrain_y {
                    max_terrain_y = terrain_y;
                }

                let surface_id_res = match biome.layers.surface_block.as_str() {
                    "sand" => sand_id,
                    "snow" => snow_id,
                    "water" => water_id,
                    _ => grass_id,
                };
                let subsurface_id_res = match biome.layers.subsurface_block.as_str() {
                    "sand" => sand_id,
                    "snow" => snow_id,
                    "dirt" => dirt_id,
                    "water" => water_id,
                    _ => dirt_id,
                };

                col_info[col] = ColInfo {
                    terrain_y,
                    subsurface_depth: biome.layers.subsurface_depth,
                    surface_id: surface_id_res,
                    subsurface_id: subsurface_id_res,
                };
            }
        }

        let mut chunk = Self {
            blocks: blocks_arr,
            x: cx,
            y: cy,
            z: cz,
        };

        // Early exit : si le chunk est complètement au-dessus du relief, il est composé d'air pur
        if cwy >= max_terrain_y {
            let t_total = t0.elapsed().as_nanos();
            let t_other_acc = t_total.saturating_sub(t_surface_acc + t_terrain_acc + t_noise_end);
            unsafe {
                static mut NOISE_IDX: usize = 0;
                static mut SURF_IDX: usize = 0;
                static mut TERR_IDX: usize = 0;
                static mut CAVE_IDX: usize = 0;
                static mut ORE_IDX: usize = 0;
                static mut VEG_IDX: usize = 0;
                static mut OTHER_IDX: usize = 0;
                gen_record_section(&mut GEN_STAT_NOISE, &mut GEN_HIST_NOISE, t_noise_end, &mut NOISE_IDX);
                gen_record_section(&mut GEN_STAT_SURFACE, &mut GEN_HIST_SURFACE, t_surface_acc, &mut SURF_IDX);
                gen_record_section(&mut GEN_STAT_TERRAIN, &mut GEN_HIST_TERRAIN, t_terrain_acc, &mut TERR_IDX);
                gen_record_section(&mut GEN_STAT_CAVE, &mut GEN_HIST_CAVE, 0, &mut CAVE_IDX);
                gen_record_section(&mut GEN_STAT_ORE, &mut GEN_HIST_ORE, 0, &mut ORE_IDX);
                gen_record_section(&mut GEN_STAT_VEGETATION, &mut GEN_HIST_VEGETATION, 0, &mut VEG_IDX);
                gen_record_section(&mut GEN_STAT_OTHER, &mut GEN_HIST_OTHER, t_other_acc, &mut OTHER_IDX);
            }
            return chunk;
        }

        // Passe grottes (masques par colonne avec sous-échantillonnage 3D et interpolation trilinéaire)
        let t_cave_start = Instant::now();
        let cave_bits = ctx.compute_cave_column_masks(cwx, cwy, cwz, &terrain_heights);
        let t_cave_acc = t_cave_start.elapsed().as_nanos();

        // Pré-filtrer les minerais par couche Y sous forme de masque de bits (zéro allocation heap)
        let mut active_ores_by_y = [0u32; CHUNK_SIZE as usize];
        for i in 0..ore_count {
            if let Some(config) = ctx.get_ore_config(i) {
                let y0 = (config.depth_min - cwy).max(0);
                let y1 = (config.depth_max - cwy).min(CHUNK_SIZE - 1);
                if y0 <= y1 {
                    let bit = 1u32 << i;
                    for y in (y0 as usize)..=(y1 as usize) {
                        active_ores_by_y[y] |= bit;
                    }
                }
            }
        }

        // Grille de rejet par boîte 4x4x4 pour les minerais — chronométré sous ORE
        let t_ore_start = Instant::now();
        let (ore_cell_mask, ore_sample_grids) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores_by_y, max_terrain_y);

        // Passe de placement : surface, sous-surface, roche et résolution des minerais
        for x in 0..CHUNK_SIZE {
            let gx = (x / 4) as usize;

            for z in 0..CHUNK_SIZE {
                let gz = (z / 4) as usize;
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);
                let info = col_info[col];

                for y in 0..CHUNK_SIZE {
                    let wy = y + cwy;
                    if wy >= info.terrain_y {
                        break;
                    }
                    if (cave_bits[col] & (1 << y)) != 0 {
                        continue;
                    }

                    let block_id_res = if wy == info.terrain_y - 1 {
                        info.surface_id
                    } else if wy >= info.terrain_y - info.subsurface_depth {
                        info.subsurface_id
                    } else {
                        let mut placed_id = stone_id;
                        let gy = (y / 4) as usize;
                        let cell_active = ore_cell_mask[gx][gz][gy];
                        let mut active = active_ores_by_y[y as usize] & cell_active;

                        if active != 0 {
                            while active != 0 {
                                let ore_idx = active.trailing_zeros() as usize;
                                active &= active - 1;

                                let is_ore = ctx.should_place_ore_approx(ore_idx, x, y, z, &ore_sample_grids[ore_idx]);

                                if is_ore {
                                    if let Some(ore_id) = ore_ids[ore_idx] {
                                        placed_id = ore_id;
                                    }
                                    break;
                                }
                            }
                        }
                        placed_id
                    };

                    chunk.set_block_from_xyz(x, y, z, BlockInstance::new(block_id_res));
                }
            }
        }

        let t_ore_acc = t_ore_start.elapsed().as_nanos();
        let t_veg_acc: u128 = 0;
        let t_other_acc = t0
            .elapsed()
            .as_nanos()
            .saturating_sub(t_surface_acc + t_terrain_acc + t_cave_acc + t_ore_acc + t_noise_end);

        unsafe {
            static mut NOISE_IDX: usize = 0;
            static mut SURF_IDX: usize = 0;
            static mut TERR_IDX: usize = 0;
            static mut CAVE_IDX: usize = 0;
            static mut ORE_IDX: usize = 0;
            static mut VEG_IDX: usize = 0;
            static mut OTHER_IDX: usize = 0;
            gen_record_section(&mut GEN_STAT_NOISE, &mut GEN_HIST_NOISE, t_noise_end, &mut NOISE_IDX);
            gen_record_section(&mut GEN_STAT_SURFACE, &mut GEN_HIST_SURFACE, t_surface_acc, &mut SURF_IDX);
            gen_record_section(&mut GEN_STAT_TERRAIN, &mut GEN_HIST_TERRAIN, t_terrain_acc, &mut TERR_IDX);
            gen_record_section(&mut GEN_STAT_CAVE, &mut GEN_HIST_CAVE, t_cave_acc, &mut CAVE_IDX);
            gen_record_section(&mut GEN_STAT_ORE, &mut GEN_HIST_ORE, t_ore_acc, &mut ORE_IDX);
            gen_record_section(&mut GEN_STAT_VEGETATION, &mut GEN_HIST_VEGETATION, t_veg_acc, &mut VEG_IDX);
            gen_record_section(&mut GEN_STAT_OTHER, &mut GEN_HIST_OTHER, t_other_acc, &mut OTHER_IDX);
        }

        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cave_mask_bitpacking_column_layout() {
        // Vérifie le layout par colonne (x,z) avec bit y
        let mut cave_bits = [0u32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];
        let x = 5;
        let z = 3;
        let col = z * (CHUNK_SIZE as usize) + x;
        let mut mask_col: u32 = 0;
        // Grottes aux y 1, 4, 7
        for y in [1u32, 4, 7].iter() {
            mask_col |= 1 << *y;
        }
        cave_bits[col] = mask_col;
        // Vérification
        assert_eq!((cave_bits[col] & (1 << 1)) != 0, true);
        assert_eq!((cave_bits[col] & (1 << 4)) != 0, true);
        assert_eq!((cave_bits[col] & (1 << 7)) != 0, true);
        assert_eq!((cave_bits[col] & (1 << 0)) != 0, false);
        assert_eq!((cave_bits[col] & (1 << 2)) != 0, false);
    }

    #[test]
    fn cave_mask_empty() {
        let cave_bits = [0u32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];
        for &m in cave_bits.iter().take((CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)) {
            assert_eq!(m, 0);
        }
    }

    #[test]
    fn active_ores_by_y_filtering() {
        // Simule le filtrage par profondeur avec un masque u32
        let mut active_ores_by_y = [0u32; CHUNK_SIZE as usize];
        let cwy = 0i32;
        let depth_min = 10i32;
        let depth_max = 15i32;
        let y0 = ((depth_min - cwy).max(0)) as usize;
        let y1 = ((depth_max - cwy).min(CHUNK_SIZE - 1)) as usize;
        let ore_bit = 1u32 << 0;
        for y in y0..=y1 {
            active_ores_by_y[y] |= ore_bit;
        }
        assert_ne!(active_ores_by_y[12] & ore_bit, 0);
        assert_eq!(active_ores_by_y[5] & ore_bit, 0);
        assert_eq!(active_ores_by_y[20] & ore_bit, 0);
    }

    #[test]
    fn ore_priority_trailing_zeros() {
        // Deux minerais 0 et 1 sont actifs simultanément sur le même bloc
        let active = (1u32 << 0) | (1u32 << 1);
        let first_match = active.trailing_zeros() as usize;
        assert_eq!(first_match, 0, "Le premier minerai (priorité 0) doit être résolu en premier");

        // Si le minerai 0 n'est pas actif, le minerai 1 doit être pris
        let active_only_1 = 1u32 << 1;
        assert_eq!(active_only_1.trailing_zeros() as usize, 1);
    }

    #[test]
    fn test_chunk_air_early_exit() {
        let mut bm = BlockManager::new();
        let blocks = crate::assets::block_loader::load_block_definitions(crate::assets::resolve_asset_path("blocks/"))
            .expect("Failed to load block definitions");
        for block in blocks {
            bm.register(block);
        }
        let bm = Arc::new(RwLock::new(bm));
        // cy = 100 est très au-dessus de tout relief terrestre
        let chunk = Chunk::generate(bm, 0, 100, 0, 42);
        for b in chunk.blocks {
            assert!(b.is_air(), "Un chunk à cy=100 doit être entièrement composé d'air");
        }
    }

    #[test]
    fn test_profiler_timing_categories_populated() {
        let mut bm = BlockManager::new();
        let blocks = crate::assets::block_loader::load_block_definitions(crate::assets::resolve_asset_path("blocks/"))
            .expect("Failed to load block definitions");
        for block in blocks {
            bm.register(block);
        }
        let bm = Arc::new(RwLock::new(bm));
        // cy = 0 est un chunk sous-terrain / niveau du sol avec surface, roche, grottes
        let chunk = Chunk::generate(bm, 0, 0, 0, 42);
        let solid_count = chunk.blocks.iter().filter(|b| b.is_solid()).count();
        assert!(solid_count > 0, "Le chunk terrestre doit contenir des blocs solides");

        unsafe {
            assert!(GEN_STAT_SURFACE.count > 0, "SURFACE doit avoir été mesuré");
            assert!(GEN_STAT_SURFACE.last_nanos > 0, "SURFACE doit avoir un timing non nul");
            assert!(GEN_STAT_TERRAIN.count > 0, "TERRAIN doit avoir été mesuré");
            assert!(GEN_STAT_TERRAIN.last_nanos > 0, "TERRAIN doit avoir un timing non nul");
            assert!(GEN_STAT_CAVE.count > 0, "CAVE doit avoir été mesuré");
            assert!(GEN_STAT_OTHER.count > 0, "OTHER doit avoir été mesuré");
        }
    }

    #[test]
    fn test_chunk_generation_with_ore_rejection_identical_blocks() {
        let mut bm = BlockManager::new();
        let blocks = crate::assets::block_loader::load_block_definitions(crate::assets::resolve_asset_path("blocks/"))
            .expect("Failed to load block definitions");
        for block in blocks {
            bm.register(block);
        }
        let bm = Arc::new(RwLock::new(bm));

        // Génère le même chunk deux fois et vérifie le déterminisme absolu
        let chunk1 = Chunk::generate(Arc::clone(&bm), 1, -1, 1, 999);
        let chunk2 = Chunk::generate(Arc::clone(&bm), 1, -1, 1, 999);
        assert_eq!(chunk1.blocks, chunk2.blocks);
    }

    #[test]
    fn test_ore_timing_includes_grid_precomputation() {
        // Vérifie que le temps ORE est > 0 pour un chunk terrestre,
        // prouvant que le pré-échantillonnage de la grille est bien inclus.
        let mut bm = BlockManager::new();
        let blocks = crate::assets::block_loader::load_block_definitions(crate::assets::resolve_asset_path("blocks/"))
            .expect("Failed to load block definitions");
        for block in blocks {
            bm.register(block);
        }
        let bm = Arc::new(RwLock::new(bm));

        // cy = -1 est un chunk profondément enfoui, il doit avoir des minerais
        let _chunk = Chunk::generate(bm, 0, -1, 0, 42);

        unsafe {
            assert!(GEN_STAT_ORE.count > 0, "ORE doit avoir été mesuré");
            assert!(
                GEN_STAT_ORE.last_nanos > 0,
                "ORE doit avoir un timing non nul (inclut le pré-échantillonnage de la grille)"
            );
        }
    }
}
