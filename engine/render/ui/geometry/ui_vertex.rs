use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub x: u32,
    pub y: u32,
    pub u: f32,
    pub v: f32,
    pub color: u32,
}

impl UiVertex {
    pub const fn new(x: u32, y: u32, u: f32, v: f32, color: u32) -> Self {
        Self { x, y, u, v, color }
    }

    pub const fn colored(x: u32, y: u32, color: u32) -> Self {
        Self::new(x, y, -1.0, -1.0, color)
    }

    pub const fn textured(x: u32, y: u32, u: f32, v: f32) -> Self {
        Self::new(x, y, u, v, 0xFFFFFFFF)
    }
}
