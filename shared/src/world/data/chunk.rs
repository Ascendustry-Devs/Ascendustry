use crate::world::data::block::BlockInstance;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const CHUNK_VALIDATION_BATCH_SIZE: usize = 20;

fn fletcher16(data: &[u8]) -> [u8; 2] {
    let mut sum1: u16 = 0;
    let mut sum2: u16 = 0;
    for &byte in data {
        sum1 = sum1.wrapping_add(byte as u16);
        sum2 = sum2.wrapping_add(sum1);
    }
    sum1 = sum1 % 255;
    sum2 = sum2 % 255;
    [(sum1 as u8), (sum2 as u8)]
}

pub const CHUNK_SIZE: i32 = 32;
pub const CHUNK_SIZE_F: f32 = CHUNK_SIZE as f32;
pub const CHUNK_SIZE_F64: f64 = CHUNK_SIZE as f64;
pub const CHUNK_SIZE_USIZE: usize = CHUNK_SIZE as usize;
pub const CHUNK_SIZE_HALFED: i32 = CHUNK_SIZE / 2;
pub const CHUNK_SIZE_SQR: i32 = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_SIZE_SQR_USIZE: usize = CHUNK_SIZE_SQR as usize;
pub const CHUNK_BLOCK_NUMBER: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;
pub const LAST_CHUNK_AXIS_INDEX: i32 = CHUNK_SIZE - 1;
pub const LAST_CHUNK_AXIS_INDEX_USIZE: usize = LAST_CHUNK_AXIS_INDEX as usize;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Serialize)]
pub enum ChunkState {
    Pending = 0,
    Ready = 1,
}

impl ChunkState {
    pub fn to_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Ready => "Ready",
        }
    }
}

pub struct ChunkData {
    pub chunk: Arc<Chunk>,
    pub state: ChunkState,
    pub is_dirty: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub blocks: Vec<BlockInstance>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntraChunkCoords {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl ChunkData {
    pub fn new(chunk: Chunk) -> Self {
        Self {
            chunk: Arc::new(chunk),
            state: ChunkState::Ready,
            is_dirty: true,
        }
    }

    pub fn set_dirty(&mut self) {
        if self.state == ChunkState::Ready {
            self.is_dirty = true;
        }
    }

    pub fn get_debug_infos(&self) -> ((i32, i32, i32), ChunkState, bool) {
        ((self.chunk.x, self.chunk.y, self.chunk.z), self.state, self.is_dirty)
    }
}

impl Chunk {
    pub fn get_block_from_xyz(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        return self.get_block_from_i((x + y * CHUNK_SIZE + z * CHUNK_SIZE_SQR) as usize);
    }

    pub fn get_block_from_i(&self, i: usize) -> BlockInstance {
        return self.blocks[i];
    }

    #[inline(always)]
    pub fn set_block_from_xyz(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) {
        self.set_block_from_i((x + y * CHUNK_SIZE + z * CHUNK_SIZE_SQR) as usize, block);
    }

    #[inline(always)]
    pub fn set_block_from_i(&mut self, i: usize, block: BlockInstance) {
        self.blocks[i] = block;
    }

    pub fn compute_checksum(&self) -> [u8; 2] {
        let bytes = bincode::serialize(&self).unwrap();
        fletcher16(&bytes)
    }
}

pub fn global_position_to_chunk_pos(gx: i32, gy: i32, gz: i32) -> ((i32, i32, i32), IntraChunkCoords) {
    let cx = gx.div_euclid(CHUNK_SIZE);
    let cy = gy.div_euclid(CHUNK_SIZE);
    let cz = gz.div_euclid(CHUNK_SIZE);

    let ix = (gx.rem_euclid(CHUNK_SIZE)) as u8;
    let iy = (gy.rem_euclid(CHUNK_SIZE)) as u8;
    let iz = (gz.rem_euclid(CHUNK_SIZE)) as u8;

    ((cx, cy, cz), IntraChunkCoords { x: ix, y: iy, z: iz })
}
