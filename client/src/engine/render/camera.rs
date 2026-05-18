use cgmath::{Matrix4, SquareMatrix};
use shared::world::data::chunk::{CHUNK_SIZE, CHUNK_SIZE_F};

use crate::common::utils::updatable::Updatable;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

pub struct RenderCamera {
    x: f32,
    y: f32,
    z: f32,
    cx: i32,
    cy: i32,
    cz: i32,
    cw: Updatable<[f32; 3]>,
    view_proj: Updatable<[[f32; 4]; 4]>,
}

impl RenderCamera {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            cx: 0,
            cy: 0,
            cz: 0,
            cw: Updatable::new([0.0, 0.0, 0.0]),
            view_proj: Updatable::new(Matrix4::identity().into()),
        }
    }

    #[inline(always)]
    pub fn update(&mut self, x: f32, y: f32, z: f32, view_proj: [[f32; 4]; 4]) {
        if x != self.x {
            self.x = x;
            self.cx = x.div_euclid(CHUNK_SIZE_F).floor() as i32;
        }
        if y != self.y {
            self.y = y;
            self.cy = y.div_euclid(CHUNK_SIZE_F).floor() as i32;
        }
        if z != self.z {
            self.z = z;
            self.cz = z.div_euclid(CHUNK_SIZE_F).floor() as i32;
        }
        let new_cw = [
            (self.cx * CHUNK_SIZE) as f32,
            (self.cy * CHUNK_SIZE) as f32,
            (self.cz * CHUNK_SIZE) as f32,
        ];
        self.cw.update_by_copy(new_cw);
        self.view_proj.update_by_copy(view_proj);
    }

    #[inline(always)]
    pub fn get_pos(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }

    #[inline(always)]
    pub fn cw(&self) -> &Updatable<[f32; 3]> {
        &self.cw
    }

    #[inline(always)]
    pub fn view_proj(&self) -> &Updatable<[[f32; 4]; 4]> {
        &self.view_proj
    }
}
