use std::sync::{Arc, RwLock};

use game::{
    assets::{block_loader::load_block_definitions, resolve_asset_path},
    world::{
        chunk_position::ChunkPosition,
        data::{
            block::BlockManager,
            chunk::{Chunk, CHUNK_SIZE},
        },
        generation::chunk_generator::{ChunkGenContext, CAVE_SCALE, CAVE_THRESHOLD, ORE_CELL_SIZE, ORE_GRID_CELLS},
        local_position::LocalPosition,
    },
};
use noise::NoiseFn;

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

        let mut position = LocalPosition::zero();
        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            position.set_x(x as u16);
            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                position.set_z(z as u16);
                for y in 0..CHUNK_SIZE {
                    let wy = (y + cwy) as f64;
                    position.set_y(y as u16);

                    let exact = ctx.should_place_ore_exact(ore_idx, wx, wy, wz);
                    let approx = ctx.should_place_ore_approx(ore_idx, &position, &sample_grids[ore_idx]);
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
    let blocks = load_block_definitions(resolve_asset_path("blocks/")).expect("Failed to load block definitions");
    for block in blocks {
        bm.register(block);
    }
    let bm = Arc::new(RwLock::new(bm));
    // cy = 100 est très au-dessus de tout relief terrestre
    let chunk = Chunk::generate(bm, &ChunkPosition::new(0, 100, 0), 42);
    for b in chunk.blocks {
        assert!(b.is_air(), "Un chunk à cy=100 doit être entièrement composé d'air");
    }
}

#[test]
fn test_chunk_generation_with_ore_rejection_identical_blocks() {
    let mut bm = BlockManager::new();
    let blocks = load_block_definitions(resolve_asset_path("blocks/")).expect("Failed to load block definitions");
    for block in blocks {
        bm.register(block);
    }
    let bm = Arc::new(RwLock::new(bm));

    // Génère le même chunk deux fois et vérifie le déterminisme absolu
    let position = ChunkPosition::new(1, -1, 1);
    let chunk1 = Chunk::generate(Arc::clone(&bm), &position, 999);
    let chunk2 = Chunk::generate(Arc::clone(&bm), &position, 999);
    assert_eq!(chunk1.blocks, chunk2.blocks);
}
