use crate::player::camera::Camera;
use crate::player::controllers::{CameraController, PlayerController};
use crate::systems::inputs::InputState;
use cgmath::{InnerSpace, Point3, Vector3, Zero};
use game::constants::{PLAYER_EYE_HEIGHT, UP};
use physics::body::PhysicsBody;
use std::f32::consts::FRAC_PI_2;
use winit::keyboard::KeyCode;

///  PLAYER EN MODE SPECTATEUR
pub struct SpectatorPlayerController {
    speed: f32,
}

impl SpectatorPlayerController {
    pub fn new(speed: f32) -> Self {
        Self { speed }
    }
}

impl PlayerController for SpectatorPlayerController {
    fn update(&self, _dt: f32, inputs: &mut InputState, body: &mut PhysicsBody, camera: &Camera) {
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

        if direction.magnitude2() > 0.0 {
            let dir = direction.normalize();
            body.velocity = dir * (self.speed);
        } else {
            body.velocity.set_zero();
        }
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
        Self { sensitivity }
    }
}

impl CameraController for FreeCameraController {
    fn update(&self, _dt: f32, inputs: &mut InputState, camera: &mut Camera, player_pos: &Point3<f32>) {
        const CLAMP_BOTTOM: f32 = -FRAC_PI_2 + 0.01;
        const CLAMP_TOP: f32 = FRAC_PI_2 - 0.01;

        // La caméra est placée à la hauteur des yeux au-dessus des pieds
        camera.set_position(*player_pos + Vector3::new(0.0, PLAYER_EYE_HEIGHT, 0.0));

        let (dx, dy) = inputs.take_mouse_delta_f32();

        camera.yaw.update_eq(|cur| cur + dx * self.sensitivity);
        camera
            .pitch
            .update_eq(|cur| (cur - dy * self.sensitivity).clamp(CLAMP_BOTTOM, CLAMP_TOP));

        if camera.needs_new_view_proj() {
            camera.set_view_proj();
            camera.set_frustum_planes();
        }
    }
}
