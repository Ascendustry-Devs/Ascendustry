use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct PositionI {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Rotation {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Position3I5 {
    inner: u16,
}

impl Position3I5 {
    pub const fn new(x: u16, y: u16, z: u16) -> Self {
        Self {
            inner: ((x & 0x1F) << 10) | ((y & 0x1F) << 5) | (z & 0x1F),
        }
    }

    pub const fn x(&self) -> u16 {
        (self.inner & 0x7C00) >> 10
    }

    pub const fn y(&self) -> u16 {
        (self.inner & 0x03E0) >> 5
    }

    pub const fn z(&self) -> u16 {
        self.inner & 0x001F
    }
}
