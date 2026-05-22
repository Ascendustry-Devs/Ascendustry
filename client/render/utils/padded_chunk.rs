use std::sync::Arc;

use crate::world::world::MeshSnapshot;
use satiscore::world::data::{
    block::BlockInstance,
    chunk::{Chunk, CHUNK_SIZE, LAST_CHUNK_AXIS_INDEX},
};

pub const PADDED_CHUNK_SIZE: i32 = CHUNK_SIZE + 2;
pub const PADDED_CHUNK_SIZE_USIZE: usize = PADDED_CHUNK_SIZE as usize;
pub const PADDED_CHUNK_SIZE_DOUBLE: usize = (PADDED_CHUNK_SIZE * 2) as usize;
pub const PADDED_CHUNK_SIZE_SQR: i32 = PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE;
pub const PADDED_CHUNK_SIZE_SQR_USIZE: usize = PADDED_CHUNK_SIZE_SQR as usize;
pub const PADDED_CHUNK_BLOCK_CBE_USIZE: usize = PADDED_CHUNK_SIZE_SQR_USIZE * PADDED_CHUNK_SIZE_USIZE;

pub const FIRST_PADDED_CHUNK_CENTER_INDEX: i32 = 1;
pub const LAST_PADDED_CHUNK_CENTER_INDEX: i32 = PADDED_CHUNK_SIZE - 2;
pub const LAST_PADDED_CHUNK_AXIS_INDEX: i32 = PADDED_CHUNK_SIZE - 1;

pub struct PaddedChunk {
    blocks: [BlockInstance; PADDED_CHUNK_BLOCK_CBE_USIZE],
}

impl PaddedChunk {
    pub const fn empty() -> PaddedChunk {
        return PaddedChunk {
            blocks: [BlockInstance::air(); PADDED_CHUNK_BLOCK_CBE_USIZE],
        };
    }

    pub fn from_snapshot(chunk: &Arc<Chunk>, snapshot: &MeshSnapshot) -> PaddedChunk {
        let mut padded_chunk = PaddedChunk::empty();

        let mut src_i: usize = 0;
        let mut dst_i = (1 + PADDED_CHUNK_SIZE + PADDED_CHUNK_SIZE_SQR) as usize;

        // Copie du main chunk
        for _z in 0..CHUNK_SIZE {
            for _y in 0..CHUNK_SIZE {
                for _x in 0..CHUNK_SIZE {
                    padded_chunk.blocks[dst_i] = chunk.get_block_from_i(src_i);
                    src_i += 1;
                    dst_i += 1;
                }
                dst_i += 2;
            }
            dst_i += PADDED_CHUNK_SIZE_DOUBLE;
        }

        // Edges - copy neighbors from snapshot
        padded_chunk.fill_edges(
            snapshot.neg_x.as_ref().map(|c| c.as_ref()),
            snapshot.pos_x.as_ref().map(|c| c.as_ref()),
            snapshot.neg_y.as_ref().map(|c| c.as_ref()),
            snapshot.pos_y.as_ref().map(|c| c.as_ref()),
            snapshot.neg_z.as_ref().map(|c| c.as_ref()),
            snapshot.pos_z.as_ref().map(|c| c.as_ref()),
        );

        padded_chunk
    }

    /// Abstraction of `get_block_from_i` but with components.
    ///
    /// Prefer using `get_block_from_i` whenever possible, as it saves computing power and time.
    ///
    /// # WARNING
    ///
    /// Use with caution, as bounds are NOT checked. May panic and abort the program if used incorrectly.
    #[inline(always)]
    pub fn get_block_from_xyz_unsafe(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        return self.get_block_from_i((x + y * PADDED_CHUNK_SIZE + z * PADDED_CHUNK_SIZE_SQR) as usize);
    }

    #[inline(always)]
    pub fn get_block_from_i(&self, i: usize) -> BlockInstance {
        return self.blocks[i];
    }

    /// Abstraction of `set_block_from_i` but with components.
    ///
    /// Prefer using `set_block_from_i` whenever possible, as it saves computing power and time.
    ///
    /// # WARNING
    ///
    /// Use with caution, as bounds are NOT checked. May panic and abort the program if used incorrectly.
    #[inline(always)]
    fn set_block_from_xyz_unsafe(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) {
        self.set_block_from_i((x + y * PADDED_CHUNK_SIZE + z * PADDED_CHUNK_SIZE_SQR) as usize, block);
    }

    #[inline(always)]
    fn set_block_from_i(&mut self, i: usize, block: BlockInstance) {
        self.blocks[i] = block;
    }

    pub fn fill_edges(
        &mut self,
        neg_x: Option<&Chunk>,
        pos_x: Option<&Chunk>,
        neg_y: Option<&Chunk>,
        pos_y: Option<&Chunk>,
        neg_z: Option<&Chunk>,
        pos_z: Option<&Chunk>,
    ) {
        if let Some(neg_x) = neg_x {
            self.fill_neg_x(neg_x);
        } else {
            self.fill_neg_x_as_air();
        }
        if let Some(pos_x) = pos_x {
            self.fill_pos_x(pos_x);
        } else {
            self.fill_pos_x_as_air();
        }
        if let Some(neg_y) = neg_y {
            self.fill_neg_y(neg_y);
        } else {
            self.fill_neg_y_as_air();
        }
        if let Some(pos_y) = pos_y {
            self.fill_pos_y(pos_y);
        } else {
            self.fill_pos_y_as_air();
        }
        if let Some(neg_z) = neg_z {
            self.fill_neg_z(neg_z);
        } else {
            self.fill_neg_z_as_air();
        }
        if let Some(pos_z) = pos_z {
            self.fill_pos_z(pos_z);
        } else {
            self.fill_pos_z_as_air();
        }
    }

    // CHUNK

    pub fn fill_neg_x(&mut self, chunk: &Chunk) {
        for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(0, y, z, chunk.get_block_from_xyz(LAST_CHUNK_AXIS_INDEX, y - 1, z - 1));
            }
        }
    }

    pub fn fill_pos_x(&mut self, chunk: &Chunk) {
        for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(LAST_PADDED_CHUNK_AXIS_INDEX, y, z, chunk.get_block_from_xyz(0, y - 1, z - 1));
            }
        }
    }

    pub fn fill_neg_y(&mut self, chunk: &Chunk) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, 0, z, chunk.get_block_from_xyz(x - 1, LAST_CHUNK_AXIS_INDEX, z - 1));
            }
        }
    }

    pub fn fill_pos_y(&mut self, chunk: &Chunk) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, LAST_PADDED_CHUNK_AXIS_INDEX, z, chunk.get_block_from_xyz(x - 1, 0, z - 1));
            }
        }
    }

    pub fn fill_neg_z(&mut self, chunk: &Chunk) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, y, 0, chunk.get_block_from_xyz(x - 1, y - 1, LAST_CHUNK_AXIS_INDEX));
            }
        }
    }

    pub fn fill_pos_z(&mut self, chunk: &Chunk) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, y, LAST_PADDED_CHUNK_AXIS_INDEX, chunk.get_block_from_xyz(x - 1, y - 1, 0));
            }
        }
    }

    // AIR

    fn fill_neg_x_as_air(&mut self) {
        for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(0, y, z, BlockInstance::air());
            }
        }
    }

    fn fill_pos_x_as_air(&mut self) {
        for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(LAST_PADDED_CHUNK_AXIS_INDEX, y, z, BlockInstance::air());
            }
        }
    }

    fn fill_neg_y_as_air(&mut self) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, 0, z, BlockInstance::air());
            }
        }
    }

    fn fill_pos_y_as_air(&mut self) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for z in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, LAST_PADDED_CHUNK_AXIS_INDEX, z, BlockInstance::air());
            }
        }
    }

    fn fill_neg_z_as_air(&mut self) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, y, 0, BlockInstance::air());
            }
        }
    }

    fn fill_pos_z_as_air(&mut self) {
        for x in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
            for y in FIRST_PADDED_CHUNK_CENTER_INDEX..=LAST_PADDED_CHUNK_CENTER_INDEX {
                self.set_block_from_xyz_unsafe(x, y, LAST_PADDED_CHUNK_AXIS_INDEX, BlockInstance::air());
            }
        }
    }
}
