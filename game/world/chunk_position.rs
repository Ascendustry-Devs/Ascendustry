use std::ops::Sub;

use serde::{Deserialize, Serialize};

use crate::{
    types::Position,
    world::{
        data::chunk::{CHUNK_SIZE, CHUNK_SIZE_F},
        world_position::WorldPosition,
    },
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPosition {
    x: i32,
    y: i32,
    z: i32,
}

impl ChunkPosition {
    #[inline(always)]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub const fn origin(&self) -> WorldPosition {
        WorldPosition::new(self.x * CHUNK_SIZE, self.y * CHUNK_SIZE, self.z * CHUNK_SIZE)
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

impl From<&WorldPosition> for ChunkPosition {
    fn from(value: &WorldPosition) -> Self {
        let (x, y, z): (i32, i32, i32) = value.into();
        Self {
            x: x.div_euclid(CHUNK_SIZE),
            y: y.div_euclid(CHUNK_SIZE),
            z: z.div_euclid(CHUNK_SIZE),
        }
    }
}

impl From<&ChunkPosition> for (i32, i32, i32) {
    fn from(value: &ChunkPosition) -> Self {
        (value.x(), value.y(), value.z())
    }
}

impl From<&Position> for ChunkPosition {
    fn from(value: &Position) -> Self {
        Self::new(
            value.x().div_euclid(CHUNK_SIZE_F).floor() as i32,
            value.y().div_euclid(CHUNK_SIZE_F).floor() as i32,
            value.z().div_euclid(CHUNK_SIZE_F).floor() as i32,
        )
    }
}

impl Sub<ChunkPosition> for ChunkPosition {
    type Output = Self;

    fn sub(self, rhs: ChunkPosition) -> Self::Output {
        Self {
            x: self.x() - rhs.x(),
            y: self.y() - rhs.y(),
            z: self.z() - rhs.z(),
        }
    }
}
