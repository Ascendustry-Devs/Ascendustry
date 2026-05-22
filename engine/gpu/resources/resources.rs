use wgpu::{BindGroup, Buffer, Texture, TextureView};

use crate::gpu::resources::pipeline::Pipelines;

pub struct GpuResources {
    pub pipelines: Pipelines,

    pub camera_bind_group: BindGroup,
    pub texture_bind_group: BindGroup,

    pub camera_buffer: Buffer,

    pub depth_texture: Texture,
    pub depth_view: TextureView,
}

impl GpuResources {
    pub fn new(
        pipelines: Pipelines,

        camera_bind_group: BindGroup,
        texture_bind_group: BindGroup,

        camera_buffer: Buffer,

        depth_texture: Texture,
        depth_view: TextureView,
    ) -> Self {
        Self {
            pipelines,
            camera_bind_group,
            texture_bind_group,
            camera_buffer,
            depth_texture,
            depth_view,
        }
    }
}
