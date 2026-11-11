use project_core::general_types::checksum::Checksum;
use project_core::math::geometry::vec3::Vec3;
use rustc_hash::{FxBuildHasher, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};

use crate::world::{
    chunk_position::ChunkPosition, data::block::BlockInstance, local_position::LocalPosition, world_position::WorldPosition,
};

pub const CHUNK_VALIDATION_BATCH_SIZE: usize = 20;

#[allow(unused)]
fn fletcher16(data: &[u8]) -> Checksum {
    let mut sum1: u16 = 0;
    let mut sum2: u16 = 0;
    for &byte in data {
        sum1 = sum1.wrapping_add(byte as u16);
        sum2 = sum2.wrapping_add(sum1);
    }
    sum1 %= 255;
    sum2 %= 255;
    [(sum1 as u8), (sum2 as u8)].into()
}

pub const CHUNK_SIZE: i32 = 32;
pub const CHUNK_SIZE_F: f32 = CHUNK_SIZE as f32;
pub const CHUNK_SIZE_F64: f64 = CHUNK_SIZE as f64;
pub const CHUNK_SIZE_USIZE: usize = CHUNK_SIZE as usize;
pub const CHUNK_SIZE_HALFED: i32 = CHUNK_SIZE / 2;
pub const CHUNK_SIZE_HALFED_VEC3_F: Vec3<f32> = Vec3::new(CHUNK_SIZE_HALFED_F, CHUNK_SIZE_HALFED_F, CHUNK_SIZE_HALFED_F);
pub const CHUNK_SIZE_HALFED_F: f32 = CHUNK_SIZE_HALFED as f32;
pub const CHUNK_SIZE_SQR: i32 = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_SIZE_SQR_USIZE: usize = CHUNK_SIZE_SQR as usize;
pub const CHUNK_SIZE_CUBE_USIZE: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;
pub const LAST_CHUNK_AXIS_INDEX: i32 = CHUNK_SIZE - 1;
pub const LAST_CHUNK_AXIS_INDEX_USIZE: usize = LAST_CHUNK_AXIS_INDEX as usize;
pub const LAST_CHUNK_AXIS_INDEX_U16: u16 = LAST_CHUNK_AXIS_INDEX as u16;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ChunkState {
    Pending = 0,
    Ready = 1,
    Meshed = 2,
}

#[derive(Debug)]
pub struct ChunkData {
    pub chunk: Arc<Chunk>,
    pub state: ChunkState,
    pub is_dirty: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub blocks: Vec<BlockInstance>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
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
        if self.state != ChunkState::Pending {
            self.is_dirty = true;
        }
    }

    pub const fn get_debug_infos(&self) -> (ChunkState, bool) {
        (self.state, self.is_dirty)
    }
}

impl Chunk {
    #[inline(always)]
    fn get_index(position: &LocalPosition) -> usize {
        let (x, y, z) = (position.x() as usize, position.y() as usize, position.z() as usize);
        x + y * CHUNK_SIZE_USIZE + z * CHUNK_SIZE_SQR_USIZE
    }

    #[inline(always)]
    pub fn get_block(&self, position: &LocalPosition) -> BlockInstance {
        self.get_block_from_index(Self::get_index(position))
    }

    #[inline(always)]
    pub fn get_block_from_index(&self, index: usize) -> BlockInstance {
        self.blocks[index]
    }

    #[inline(always)]
    pub fn set_block(&mut self, position: &LocalPosition, block: BlockInstance) -> Option<BlockInstance> {
        self.set_block_from_index(Self::get_index(position), block)
    }

    #[inline(always)]
    pub fn set_block_from_index(&mut self, i: usize, block: BlockInstance) -> Option<BlockInstance> {
        let old = self.blocks[i];
        if old == block {
            return None;
        }
        self.blocks[i] = block;
        Some(old)
    }

    pub fn neighbors_from_block_pos(position: &WorldPosition) -> Vec<(i32, i32, i32)> {
        let mut neighbors = Vec::with_capacity(3);
        let (chunk_position, local_position) = position.into();
        let (cx, cy, cz) = (&chunk_position).into();
        let (lx, ly, lz) = (&local_position).into();

        if lx == 0 {
            neighbors.push((cx - 1, cy, cz));
        } else if lx == LAST_CHUNK_AXIS_INDEX_U16 {
            neighbors.push((cx + 1, cy, cz));
        }

        if ly == 0 {
            neighbors.push((cx, cy - 1, cz));
        } else if ly == LAST_CHUNK_AXIS_INDEX_U16 {
            neighbors.push((cx, cy + 1, cz));
        }

        if lz == 0 {
            neighbors.push((cx, cy, cz - 1));
        } else if lz == LAST_CHUNK_AXIS_INDEX_U16 {
            neighbors.push((cx, cy, cz + 1));
        }

        neighbors
    }

    pub fn compute_checksum(&self) -> Checksum {
        let mut sum1: u16 = 0;
        let mut sum2: u16 = 0;

        for &coord in &[self.x, self.y, self.z] {
            let rep = coord as u32;
            sum1 = sum1.wrapping_add(rep as u16);
            sum2 = sum2.wrapping_add(sum1);
        }

        for block in &self.blocks {
            let rep = block.get_block_id();
            sum1 = sum1.wrapping_add(Into::<u32>::into(rep) as u16);
            sum2 = sum2.wrapping_add(sum1);
        }

        sum1 %= 255;
        sum2 %= 255;
        [(sum1 as u8), (sum2 as u8)].into()
    }

    pub fn get_cube_chunk_range(center: &ChunkPosition, hd: u16, vd: u16) -> [i32; 6] {
        let halfed_hd = hd.div_euclid(2) as i32;
        let halfed_vd = vd.div_euclid(2) as i32;

        let (cx, cy, cz) = center.into();

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
    ) -> Vec<ChunkPosition> {
        let chunk_number = ((max_cx - min_cx) * (max_cy - min_cy) * (max_cz - min_cz)) as usize;
        let mut keys = Vec::with_capacity(chunk_number);

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    keys.push(ChunkPosition::new(x, y, z));
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
        // Les bornes sont inclusives : la capacité doit couvrir
        // (max - min + 1) valeurs par axe, sinon la table de hash doit
        // être redimensionnée en cours de remplissage.
        let chunk_number = ((max_cx - min_cx + 1) * (max_cy - min_cy + 1) * (max_cz - min_cz + 1)) as usize;
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
