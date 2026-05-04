use cgmath::Matrix4;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderCamera {
    x: f32,
    y: f32,
    z: f32,
    view_proj: [[f32; 4]; 4],
}

impl RenderCamera {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update(&mut self, x: f32, y: f32, z: f32, view_proj: [[f32; 4]; 4]) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.view_proj = view_proj;
    }

    pub fn get_pos(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }

    pub fn get_view_proj(&self) -> [[f32; 4]; 4] {
        self.view_proj
    }
}
