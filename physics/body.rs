use crate::aabb::AABB;
use game::{
    constants::{GRAVITY, JUMP_SPEED, WALK_SPEED},
    types::Position,
};
use project_core::general_types::{updatable::Updatable, vec3::Vec3};

pub struct PhysicsBody {
    pub aabb: AABB,
    pub velocity: Updatable<Vec3<f32>>,
    pub on_ground: bool,
    pub gravity: f32,
    pub jump_speed: f32,
    pub walk_speed: f32,
}

impl PhysicsBody {
    pub fn new(center: Position, half_size: f32) -> Self {
        Self {
            aabb: AABB::from_cube(center, half_size),
            velocity: Updatable::new(Vec3::new(0.0, 0.0, 0.0)),
            on_ground: false,
            gravity: GRAVITY,
            jump_speed: JUMP_SPEED,
            walk_speed: WALK_SPEED,
        }
    }

    pub const fn velocity_mut(&mut self) -> &mut Updatable<Vec3<f32>> {
        &mut self.velocity
    }
}
