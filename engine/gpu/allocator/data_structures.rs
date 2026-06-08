use crate::gpu::allocator::gpu_allocator::EntryId;
use std::fmt::Display;

#[repr(u8)]
#[derive(Debug)]
pub enum AllocError {
    InvalidId = 0,
    NotEnoughSpace = 1,
}

#[derive(Clone)]
pub struct Gap {
    pub position: usize,
    pub length: usize,
}

pub struct WriteOperation {
    pub mesh_id: EntryId,
    pub offset: usize,
    pub len: usize,
    pub arena_offset: usize,
}

impl Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            Self::InvalidId => "InvalidId",
            Self::NotEnoughSpace => "NotEnoughSpace",
        })
    }
}

impl Gap {
    pub fn new(position: usize, length: usize) -> Self {
        Self { position, length }
    }
}

impl WriteOperation {
    pub fn new(mesh_id: EntryId, offset: usize, len: usize, arena_offset: usize) -> Self {
        Self {
            mesh_id,
            offset,
            len,
            arena_offset,
        }
    }
}
