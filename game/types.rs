use project_core::{math::geometry::space::Pos3, math::geometry::vec3::Vec3};
use serde::{Deserialize, Serialize};

use crate::world::chunk_position::ChunkPosition;

// miss_example
/// Represents a **Position** in 3D space by its 3 coordinates : `x`, `y` and `z`.
pub type Position = Pos3<f32>;

// miss_example
/// Represents a z-axis-missing **Rotation** in 3D space by its yaw (x) and pitch (y).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}

impl Rotation {
    // miss_example
    /// Creates a `Rotation`.
    pub const fn new(yaw: f32, pitch: f32) -> Self {
        Self { yaw, pitch }
    }

    // miss_example
    /// Returns the `yaw` of current `Rotation`.
    pub const fn yaw(&self) -> f32 {
        self.yaw
    }

    // miss_example
    /// Returns the `pitch` of current `Rotation`.
    pub const fn pitch(&self) -> f32 {
        self.pitch
    }
}

impl From<&Rotation> for (f32, f32) {
    fn from(value: &Rotation) -> Self {
        (value.yaw, value.pitch)
    }
}

impl From<&ChunkPosition> for Position {
    fn from(value: &ChunkPosition) -> Self {
        let x = (value.x() << 5) as f32;
        let y = (value.y() << 5) as f32;
        let z = (value.z() << 5) as f32;

        let inner = Vec3::new(x, y, z);
        Self(inner)
    }
}
