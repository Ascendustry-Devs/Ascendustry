use crate::aabb::AABB;
use cgmath::{Point3, Vector3};
use satiscore::constants::{GRAVITY, JUMP_SPEED, WALK_SPEED};

pub struct PhysicsBody {
    pub aabb: AABB,
    pub velocity: Vector3<f32>,
    pub on_ground: bool,
    pub gravity: f32,
    pub jump_speed: f32,
    pub walk_speed: f32,
}

impl PhysicsBody {
    pub fn new(center: Point3<f32>, half_size: f32) -> Self {
        Self {
            aabb: AABB::new(center, half_size),
            velocity: Vector3::new(0.0, 0.0, 0.0),
            on_ground: false,
            gravity: GRAVITY,
            jump_speed: JUMP_SPEED,
            walk_speed: WALK_SPEED,
        }
    }
}
