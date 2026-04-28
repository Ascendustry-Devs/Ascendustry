use cgmath::{Deg, InnerSpace, Matrix4, Point3, Vector3};

use crate::{engine::render::camera::OPENGL_TO_WGPU_MATRIX, game::systems::inputs::InputState};

#[derive(Clone)]
pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub fovy: f32,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

pub trait CameraController {
    fn update(&self, dt: f32, inputs: &mut InputState, camera: &mut Camera, player_pos: &Point3<f32>);
}

impl Camera {
    pub fn new(eye: cgmath::Point3<f32>, aspect: f32) -> Camera {
        Camera {
            eye,
            yaw: 0.0,
            pitch: 0.0,
            fovy: 70.0,
            aspect,
            znear: 0.1,
            zfar: 1000.0,
        }
    }

    pub fn forward(&self) -> Vector3<f32> {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();

        Vector3::new(cy * cp, sp, sy * cp).normalize()
    }

    pub fn right(&self) -> Vector3<f32> {
        self.forward().cross(Vector3::unit_y()).normalize()
    }

    pub fn target(&self) -> Point3<f32> {
        self.eye + self.forward()
    }

    pub fn get_view_proj(&self) -> Matrix4<f32> {
        let view = Matrix4::look_at_rh(self.eye, self.target(), Vector3::unit_y());
        let proj = cgmath::perspective(Deg(self.fovy), self.aspect, self.znear, self.zfar);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }

    pub fn set_position(&mut self, position: cgmath::Point3<f32>) {
        self.eye = position;
    }

    pub fn get_rotation(&self) -> (f32, f32) {
        (self.yaw, self.pitch)
    }
}