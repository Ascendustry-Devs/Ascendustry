use cgmath::Vector3;
use rustc_hash::{FxBuildHasher, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Display, sync::Arc};

use crate::world::data::block::BlockInstance;

pub const CHUNK_VALIDATION_BATCH_SIZE: usize = 20;

fn fletcher16(data: &[u8]) -> [u8; 2] {
    let mut sum1: u16 = 0;
    let mut sum2: u16 = 0;
    for &byte in data {
        sum1 = sum1.wrapping_add(byte as u16);
        sum2 = sum2.wrapping_add(sum1);
    }
    sum1 %= 255;
    sum2 %= 255;
    [(sum1 as u8), (sum2 as u8)]
}

pub const CHUNK_SIZE: i32 = 32;
pub const CHUNK_SIZE_F: f32 = CHUNK_SIZE as f32;
pub const CHUNK_SIZE_F64: f64 = CHUNK_SIZE as f64;
pub const CHUNK_SIZE_USIZE: usize = CHUNK_SIZE as usize;
pub const CHUNK_SIZE_HALFED: i32 = CHUNK_SIZE / 2;
pub const CHUNK_SIZE_HALFED_VEC3_F: Vector3<f32> = Vector3::new(CHUNK_SIZE_HALFED_F, CHUNK_SIZE_HALFED_F, CHUNK_SIZE_HALFED_F);
pub const CHUNK_SIZE_HALFED_F: f32 = CHUNK_SIZE_HALFED as f32;
pub const CHUNK_SIZE_SQR: i32 = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_SIZE_SQR_USIZE: usize = CHUNK_SIZE_SQR as usize;
pub const CHUNK_BLOCK_NUMBER: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;
pub const LAST_CHUNK_AXIS_INDEX: i32 = CHUNK_SIZE - 1;
pub const LAST_CHUNK_AXIS_INDEX_USIZE: usize = LAST_CHUNK_AXIS_INDEX as usize;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum ChunkState {
    Pending = 0,
    Ready = 1,
}

impl Display for ChunkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "Pending",
            Self::Ready => "Ready",
        })
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

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

    pub fn get_debug_infos(&self) -> (ChunkState, bool) {
        (self.state, self.is_dirty)
    }
}

impl Chunk {
    pub fn get_block_from_xyz(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        self.get_block_from_i((x + y * CHUNK_SIZE + z * CHUNK_SIZE_SQR) as usize)
    }

    pub fn get_block_from_i(&self, i: usize) -> BlockInstance {
        self.blocks[i]
    }

    #[inline(always)]
    pub fn set_block_from_xyz(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) {
        self.set_block_from_i((x + y * CHUNK_SIZE + z * CHUNK_SIZE_SQR) as usize, block);
    }

    #[inline(always)]
    pub fn set_block_from_i(&mut self, i: usize, block: BlockInstance) {
        self.blocks[i] = block;
    }

    pub fn chunk_coords_from_world(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (x.div_euclid(CHUNK_SIZE), y.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
    }

    pub fn local_coords_from_world(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (x.rem_euclid(CHUNK_SIZE), y.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE))
    }

    pub fn neighbors_from_block_pos(x: i32, y: i32, z: i32) -> Vec<(i32, i32, i32)> {
        let mut neighbors = Vec::new();
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        let (lx, ly, lz) = Chunk::local_coords_from_world(x, y, z);

        if lx == 0 {
            neighbors.push((cx - 1, cy, cz));
        } else if lx == LAST_CHUNK_AXIS_INDEX {
            neighbors.push((cx + 1, cy, cz));
        }

        if ly == 0 {
            neighbors.push((cx, cy - 1, cz));
        } else if ly == LAST_CHUNK_AXIS_INDEX {
            neighbors.push((cx, cy + 1, cz));
        }

        if lz == 0 {
            neighbors.push((cx, cy, cz - 1));
        } else if lz == LAST_CHUNK_AXIS_INDEX {
            neighbors.push((cx, cy, cz + 1));
        }

        neighbors
    }

    pub fn compute_checksum(&self) -> [u8; 2] {
        let mut sum1: u16 = 0;
        let mut sum2: u16 = 0;

        for &coord in &[self.x, self.y, self.z] {
            let rep = coord as u32;
            sum1 = sum1.wrapping_add(rep as u16);
            sum2 = sum2.wrapping_add(sum1);
        }

        for block in &self.blocks {
            let rep = block.get_block_id();
            sum1 = sum1.wrapping_add(rep as u16);
            sum2 = sum2.wrapping_add(sum1);
        }

        sum1 %= 255;
        sum2 %= 255;
        [(sum1 as u8), (sum2 as u8)]
    }

    pub const fn get_cube_chunk_range(center: (i32, i32, i32), hd: u16, vd: u16) -> [i32; 6] {
        let halfed_hd = hd.div_euclid(2) as i32;
        let halfed_vd = vd.div_euclid(2) as i32;

        let (cx, cy, cz) = center;

        let min_cx = cx - halfed_hd;
        let max_cx = cx + halfed_hd;
        let min_cy = cy - halfed_vd;
        let max_cy = cy + halfed_vd;
        let min_cz = cz - halfed_hd;
        let max_cz = cz + halfed_hd;

        [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    }

    pub fn get_cube_chunk_keys(
        min_cx: i32,
        max_cx: i32,
        min_cy: i32,
        max_cy: i32,
        min_cz: i32,
        max_cz: i32,
    ) -> Vec<(i32, i32, i32)> {
        let chunk_number = ((max_cx - min_cx) * (max_cy - min_cy) * (max_cz - min_cz)) as usize;
        let mut keys = Vec::with_capacity(chunk_number);

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    keys.push((x, y, z));
                }
            }
        }

        keys
    }

    pub fn get_cube_chunk_keys_set(
        min_cx: i32,
        max_cx: i32,
        min_cy: i32,
        max_cy: i32,
        min_cz: i32,
        max_cz: i32,
    ) -> FxHashSet<(i32, i32, i32)> {
        let chunk_number = ((max_cx - min_cx) * (max_cy - min_cy) * (max_cz - min_cz)) as usize;
        let mut keys = HashSet::with_capacity_and_hasher(chunk_number, FxBuildHasher);

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    keys.insert((x, y, z));
                }
            }
        }

        keys
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
