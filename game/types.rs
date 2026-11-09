use project_core::{general_types::vec3::Vec3, math::geometry::space::Pos3};
use serde::{Deserialize, Serialize};

use crate::world::chunk_position::ChunkPosition;

// pub struct Position {
//     pub x: f32,
//     pub y: f32,
//     pub z: f32,
// }
pub type Position = Pos3<f32>;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    pub x: f32,
    pub y: f32,
}

impl Rotation {
    pub fn new(yaw: f32, pitch: f32) -> Self {
        Self { x: yaw, y: pitch }
    }
}

impl From<&Rotation> for (f32, f32) {
    fn from(value: &Rotation) -> Self {
        (value.x, value.y)
    }
}

impl From<&ChunkPosition> for Position {
    fn from(value: &ChunkPosition) -> Self {
        let x = (value.x() << 5) as f32;
        let y = (value.y() << 5) as f32;
        let z = (value.z() << 5) as f32;

        let inner = Vec3::new(x, y, z);
        Self { 0: inner }
    }
}
