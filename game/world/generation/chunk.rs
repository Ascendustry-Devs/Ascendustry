#![allow(static_mut_refs)]
use std::sync::{Arc, RwLock};

use noise::NoiseFn;
use project_core::general_types::checksum::Checksum;

use crate::world::chunk_position::ChunkPosition;
use crate::world::data::block::{BlockId, BlockInstance, BlockManager};
use crate::world::data::chunk::{Chunk, ChunkData, CHUNK_SIZE, CHUNK_SIZE_CUBE_USIZE};
use crate::world::generation::chunk_generator::ChunkGenContext;
use crate::world::local_position::LocalPosition;

#[derive(Debug)]
pub struct ChunkWithChecksum {
    pub chunk_data: ChunkData,
    pub checksum: Checksum,
}

impl Chunk {
    #[inline]
    pub fn generate(block_manager: Arc<RwLock<BlockManager>>, position: &ChunkPosition, seed: u32) -> Self {
        let ctx = ChunkGenContext::new(seed, block_manager);
        Self::generate_with_context(position, &ctx)
    }

    #[inline]
    pub fn generate_with_context(chunk_position: &ChunkPosition, ctx: &ChunkGenContext) -> Self {
        let chunk_origin = chunk_position.origin();
        let (cwx, cwy, cwz) = (&chunk_origin).into();

        let blocks = ctx.block_manager.read().unwrap();

        let block_id = |name: &str| -> BlockId {
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
        let mut ore_ids: [Option<BlockId>; 32] = [None; 32];
        for (i, ore_id) in ore_ids.iter_mut().enumerate().take(ore_count) {
            if let Some(config) = ctx.get_ore_config(i) {
                *ore_id = blocks.get_block_by_string(config.block_id.clone()).map(|b| b.get_id());
            }
        }

        drop(blocks);

        let blocks_arr = vec![BlockInstance::air(); CHUNK_SIZE_CUBE_USIZE];

        #[derive(Clone, Copy)]
        struct ColInfo {
            terrain_y: i32,
            subsurface_depth: i32,
            surface_id: BlockId,
            subsurface_id: BlockId,
        }

        let mut col_info = [ColInfo {
            terrain_y: 0,
            subsurface_depth: 3,
            surface_id: grass_id,
            subsurface_id: dirt_id,
        }; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];

        let mut terrain_heights = [0i32; (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)];
        let mut max_terrain_y = i32::MIN;

        // Pré-calcul unique par colonne (biome + terrain + ids)
        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);

                let biome_idx = ctx.get_biome_index(wx, wz);
                let biome = &ctx.biome_registry.biomes[biome_idx];

                let nx = wx * biome.terrain.scale;
                let nz = wz * biome.terrain.scale;
                let valeur = ctx.surface.get([nx, nz]);
                let terrain_y = valeur.mul_add(biome.terrain.amplitude, biome.terrain.base_height) as i32;

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
            x: chunk_position.x(),
            y: chunk_position.y(),
            z: chunk_position.z(),
        };

        // Early exit : si le chunk est complètement au-dessus du relief, il est composé d'air pur
        if cwy >= max_terrain_y {
            return chunk;
        }

        // Passe grottes (masques par colonne avec sous-échantillonnage 3D et interpolation trilinéaire)
        let cave_bits = ctx.compute_cave_column_masks(cwx, cwy, cwz, &terrain_heights);

        // Pré-filtrer les minerais par couche Y sous forme de masque de bits (zéro allocation heap)
        let mut active_ores_by_y = [0u32; CHUNK_SIZE as usize];
        for i in 0..ore_count {
            if let Some(config) = ctx.get_ore_config(i) {
                let y0 = (config.depth_min - cwy).max(0);
                let y1 = (config.depth_max - cwy).min(CHUNK_SIZE - 1);
                if y0 <= y1 {
                    let bit = 1u32 << i;
                    for active_ore_by_y in active_ores_by_y.iter_mut().take(y1 as usize + 1).skip(y0 as usize) {
                        *active_ore_by_y |= bit;
                    }
                }
            }
        }

        // Grille de rejet par boîte 4x4x4 pour les minerais — chronométré sous ORE
        let (ore_cell_mask, ore_sample_grids) = ctx.compute_ore_rejection_grid(cwx, cwy, cwz, &active_ores_by_y, max_terrain_y);

        // Passe de placement : surface, sous-surface, roche et résolution des minerais
        let mut position = LocalPosition::zero();
        for x in 0..CHUNK_SIZE {
            let gx = (x / 4) as usize;
            position.set_x(x as u16);

            for z in 0..CHUNK_SIZE {
                let gz = (z / 4) as usize;
                let col = (z as usize) * (CHUNK_SIZE as usize) + (x as usize);
                let info = col_info[col];
                position.set_z(z as u16);

                for y in 0..CHUNK_SIZE {
                    let wy = y + cwy;
                    if wy >= info.terrain_y {
                        break;
                    }
                    if (cave_bits[col] & (1 << y)) != 0 {
                        continue;
                    }
                    position.set_y(y as u16);

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

                                let is_ore = ctx.should_place_ore_approx(ore_idx, &position, &ore_sample_grids[ore_idx]);

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

                    chunk.set_block(&position, BlockInstance::new(block_id_res));
                }
            }
        }

        chunk
    }
}
