use std::fmt::{Display, Formatter, Result};

use project_core::general_types::bit::Bit;
use serde::{Deserialize, Serialize};

use crate::world::{data::chunk::CHUNK_SIZE, world_position::WorldPosition};

const COORD_BITS: u32 = 5;
const X_PADDING: u32 = 15;
const Y_PADDING: u32 = 10;
const Z_PADDING: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalPosition {
    inner: u16,
}

impl LocalPosition {
    #[inline(always)]
    pub const fn zero() -> Self {
        Self { inner: 0 }
    }

    #[inline(always)]
    pub fn new(x: u16, y: u16, z: u16) -> Self {
        assert!(x < 32 && y < 32 && z < 32);
        let mut instance = Self { inner: 0 };
        instance.set_x(x);
        instance.set_y(y);
        instance.set_z(z);
        instance
    }

    #[inline(always)]
    pub fn x(&self) -> u16 {
        Bit::get_part_of(self.inner, X_PADDING, COORD_BITS)
    }

    #[inline(always)]
    pub fn y(&self) -> u16 {
        Bit::get_part_of(self.inner, Y_PADDING, COORD_BITS)
    }

    #[inline(always)]
    pub fn z(&self) -> u16 {
        Bit::get_part_of(self.inner, Z_PADDING, COORD_BITS)
    }

    #[inline(always)]
    pub fn set_x(&mut self, x: u16) {
        self.inner = Bit::set_part_of(self.inner, X_PADDING, COORD_BITS, x);
    }

    #[inline(always)]
    pub fn set_y(&mut self, y: u16) {
        self.inner = Bit::set_part_of(self.inner, Y_PADDING, COORD_BITS, y);
    }

    #[inline(always)]
    pub fn set_z(&mut self, z: u16) {
        self.inner = Bit::set_part_of(self.inner, Z_PADDING, COORD_BITS, z);
    }
}

impl Display for LocalPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "LocalPosition({}, {}, {})", self.x(), self.y(), self.z())
    }
}

impl From<&WorldPosition> for LocalPosition {
    fn from(value: &WorldPosition) -> Self {
        let (x, y, z): (i32, i32, i32) = value.into();
        Self::new(
            x.rem_euclid(CHUNK_SIZE).unsigned_abs() as u16,
            y.rem_euclid(CHUNK_SIZE).unsigned_abs() as u16,
            z.rem_euclid(CHUNK_SIZE).unsigned_abs() as u16,
        )
    }
}

impl From<&LocalPosition> for (u16, u16, u16) {
    fn from(value: &LocalPosition) -> Self {
        (value.x(), value.y(), value.z())
    }
}
