use std::fmt::{Display, Formatter, Result};

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

impl Display for WorldPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "WorldPosition({}, {}, {})", self.x, self.y, self.z)
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
