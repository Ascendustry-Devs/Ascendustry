use std::sync::{Arc, RwLock};

use noise::NoiseFn;

use crate::world::data::block::{BlockInstance, BlockManager};
use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER, CHUNK_SIZE};
use crate::world::generation::chunk_generator::ChunkGenContext;

pub const TERRAIN_SCALE: f64 = 0.017;
pub const TERRAIN_BASE_HEIGHT: f64 = 0.0;
pub const TERRAIN_AMPLITUDE: f64 = 12.0;

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
        let cwx = cx * CHUNK_SIZE;
        let cwy = cy * CHUNK_SIZE;
        let cwz = cz * CHUNK_SIZE;

        let blocks = ctx.block_manager.read().unwrap();

        let grass_id = blocks
            .get_block_by_string(String::from("grass"))
            .expect("Did not find block 'grass' in block manager")
            .get_id();
        let dirt_id = blocks
            .get_block_by_string(String::from("dirt"))
            .expect("Did not find block 'dirt' in block manager")
            .get_id();
        let stone_id = blocks
            .get_block_by_string(String::from("stone"))
            .expect("Did not find block 'stone' in block manager")
            .get_id();

        let mut ore_ids: Vec<Option<u32>> = Vec::with_capacity(ctx.get_ore_count());
        for i in 0..ctx.get_ore_count() {
            if let Some(config) = ctx.get_ore_config(i) {
                ore_ids.push(blocks.get_block_by_string(config.block_id.clone()).map(|b| b.get_id()));
            } else {
                ore_ids.push(None);
            }
        }

        drop(blocks);

        let blocks = vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER];

        let mut chunk = Self {
            blocks,
            x: cx,
            y: cy,
            z: cz,
        };

        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            let nx = wx * TERRAIN_SCALE;

            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                let nz = wz * TERRAIN_SCALE;

                let valeur = ctx.surface.get([nx, nz]);
                let terrain_y = valeur.mul_add(TERRAIN_AMPLITUDE, TERRAIN_BASE_HEIGHT) as i32;

                for y in 0..CHUNK_SIZE {
                    let wy = y + cwy;
                    if wy >= terrain_y {
                        continue;
                    }

                    let depth = terrain_y - wy;

                    let is_cave = ctx.is_cave_block(wx, wy as f64, wz, depth);

                    if !is_cave {
                        let block_id = match wy {
                            y if y == terrain_y - 1 => grass_id,
                            y if y >= terrain_y - 4 => dirt_id,
                            _ => {
                                let mut placed_id = stone_id;
                                for i in 0..ctx.get_ore_count() {
                                    if let Some(config) = ctx.get_ore_config(i) {
                                        if wy >= config.depth_min && wy <= config.depth_max {
                                            if ctx.should_place_ore(i, wx, wy as f64, wz) {
                                                if let Some(ore_id) = ore_ids[i] {
                                                    placed_id = ore_id;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                                placed_id
                            }
                        };
                        chunk.set_block_from_xyz(x, y, z, BlockInstance::new(block_id));
                    }
                }
            }
        }

        chunk
    }
}
