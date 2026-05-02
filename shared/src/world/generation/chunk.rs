use std::sync::Arc;

use crate::world::data::block::{BlockInstance, BlockManager, BlockType};
use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER, CHUNK_SIZE};
use crate::world::generation::chunk_generator::ChunkGenContext;
use noise::NoiseFn;

pub struct ChunkWithChecksum {
    pub chunk_data: crate::world::data::chunk::ChunkData,
    pub checksum: [u8; 2],
}

impl Chunk {
    #[inline]
    pub fn generate(block_manager: Arc<BlockManager>, cx: i32, cy: i32, cz: i32, seed: u32) -> Chunk {
        let ctx = ChunkGenContext::new(seed, block_manager);
        Self::generate_with_context(cx, cy, cz, &ctx)
    }

    #[inline]
    pub fn generate_with_context(cx: i32, cy: i32, cz: i32, ctx: &ChunkGenContext) -> Chunk {
        const SCALE: f64 = 0.02;
        const BASE_HEIGHT: f64 = 0.0;
        const AMPLITUDE: f64 = 10.0;

        let cwx = cx * CHUNK_SIZE;
        let cwy = cy * CHUNK_SIZE;
        let cwz = cz * CHUNK_SIZE;

        let grass_id = ctx
            .block_manager
            .get_block_by_string(String::from("grass"))
            .expect("Did not find block 'grass' in block manager")
            .get_id();
        let dirt_id = ctx
            .block_manager
            .get_block_by_string(String::from("dirt"))
            .expect("Did not find block 'dirt' in block manager")
            .get_id();
        let stone_id = ctx
            .block_manager
            .get_block_by_string(String::from("stone"))
            .expect("Did not find block 'stone' in block manager")
            .get_id();

        let blocks = vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER];

        let mut chunk = Chunk {
            blocks,
            x: cx,
            y: cy,
            z: cz,
        };

        for x in 0..CHUNK_SIZE {
            let wx = (x + cwx) as f64;
            let nx = wx * SCALE;

            for z in 0..CHUNK_SIZE {
                let wz = (z + cwz) as f64;
                let nz = wz * SCALE;

                let valeur = ctx.perlin.get([nx, nz]);
                let terrain_y = (BASE_HEIGHT + valeur * AMPLITUDE) as i32;

                for y in 0..CHUNK_SIZE {
                    let wy = y + cwy;
                    if wy >= terrain_y {
                        continue;
                    }

                    let depth = terrain_y - wy;

                    let is_cave = ctx.is_cave_block(wx, wy as f64, wz, depth);

                    // "if is_cave" branch is not needed. Blocks are by default air.
                    if !is_cave {
                        let block_id = match wy {
                            y if y == terrain_y - 1 => grass_id,
                            y if y >= terrain_y - 4 => dirt_id,
                            _ => stone_id,
                        };
                        chunk.set_block_from_xyz(x, y, z, BlockInstance::new(block_id));
                    }
                }
            }
        }

        chunk
    }
}
