use crate::{player::camera::Camera, systems::inputs::InputState};
use cgmath::Point3;
use physics::body::PhysicsBody;

pub mod spectator;
pub mod walk;

pub trait PlayerController {
    fn update(&self, dt: f32, inputs: &mut InputState, body: &mut PhysicsBody, camera: &Camera);
}

pub trait CameraController {
    fn update(&self, dt: f32, inputs: &mut InputState, camera: &mut Camera, player_pos: &Point3<f32>);
}
