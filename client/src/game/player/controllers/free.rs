use std::f32::consts::FRAC_PI_2;

use cgmath::{InnerSpace, Point3, Vector3};
use winit::keyboard::KeyCode;

use crate::game::{systems::inputs::InputState, player::{camera::{Camera, CameraController}, player::PlayerController}};

//
//
//  PLAYER
//
//

pub struct FreePlayerController {
    speed: f32
}

impl FreePlayerController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed
        }
    }
}

impl PlayerController for FreePlayerController {
    fn update(
        &self,
        dt: f32,
        inputs: &mut InputState,
        player_pos: &mut Point3<f32>,
        camera: &Camera
    ) {
        const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);
        let forward = camera.forward();
        let right = camera.right();

        let mut direction = Vector3::new(0.0, 0.0, 0.0);

        if inputs.is_key_pressed(KeyCode::KeyW) {
            direction += forward;
        }
        if inputs.is_key_pressed(KeyCode::KeyS) {
            direction -= forward
        }
        if inputs.is_key_pressed(KeyCode::KeyD) {
            direction += right;
        }
        if inputs.is_key_pressed(KeyCode::KeyA) {
            direction -= right;
        }
        if inputs.is_key_pressed(KeyCode::Space) {
            direction += UP;
        }
        if inputs.is_key_pressed(KeyCode::ShiftLeft) {
            direction -= UP;
        }

        let velocity: Vector3<f32>;

        if direction.magnitude2() > 0.0 {
            velocity = direction.normalize() * (self.speed * dt);
        } else {
            velocity = Vector3::new(0.0, 0.0, 0.0);
        }

        *player_pos = *player_pos + velocity;
    }
}

//
//
//  CAMERA
//
//

pub struct FreeCameraController {
    sensitivity: f32,
}

impl FreeCameraController {
    pub fn new(sensitivity: f32) -> Self {
        Self {
            sensitivity,
        }
    }
}

impl CameraController for FreeCameraController {
    fn update(
        &self,
        dt: f32,
        inputs: &mut InputState,
        camera: &mut Camera,
        player_pos: &Point3<f32>
    ) {
        const CLAMP_BOTTOM: f32 = -FRAC_PI_2 + 0.01;
        const CLAMP_TOP: f32 = FRAC_PI_2 - 0.01;

        camera.set_position(player_pos.clone());

        let (dx, dy) = inputs.take_mouse_delta_f32();
        let coefficient = self.sensitivity * dt;

        camera.yaw += dx * coefficient;
        camera.pitch -= dy * coefficient;
        camera.pitch = camera.pitch.clamp(CLAMP_BOTTOM, CLAMP_TOP);
    }
}