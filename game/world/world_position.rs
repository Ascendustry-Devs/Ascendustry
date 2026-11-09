use serde::{Deserialize, Serialize};

use crate::world::{chunk_position::ChunkPosition, local_position::LocalPosition};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WorldPosition {
    x: i32,
    y: i32,
    z: i32,
}

impl WorldPosition {
    #[inline(always)]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub fn x(&self) -> i32 {
        self.x
    }

    #[inline(always)]
    pub fn y(&self) -> i32 {
        self.y
    }

    #[inline(always)]
    pub fn z(&self) -> i32 {
        self.z
    }
}

impl From<&WorldPosition> for (ChunkPosition, LocalPosition) {
    fn from(value: &WorldPosition) -> Self {
        (ChunkPosition::from(value), LocalPosition::from(value))
    }
}

impl From<&WorldPosition> for (i32, i32, i32) {
    fn from(value: &WorldPosition) -> Self {
        (value.x(), value.y(), value.z())
    }
}

// impl From<&ChunkPosition> for Position {
//     fn from(value: &ChunkPosition) -> Self {
//         let (x, y, z) = value.into();
//         Self {
//             x: x * CHUNK_SIZE,
//             y: y * CHUNK_SIZE,
//             z: z * CHUNK_SIZE,
//         }
//     }
// }
