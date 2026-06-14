use std::sync::Arc;

use bytemuck::cast_slice;
use cgmath::ortho;
use wgpu::{BindGroup, Buffer, BufferDescriptor, BufferUsages, RenderPass, RenderPipeline};

use crate::{
    gpu::{smart_buffer::SmartBuffer, tools::GpuTools},
    render::ui::geometry::ui_vertex::UiVertex,
};

pub struct UiRenderer {
    gpu_tools: Arc<GpuTools>,
    vertex_buffer: SmartBuffer,
    projection_buffer: Buffer,
}

impl UiRenderer {
    pub fn new(gpu_tools: Arc<GpuTools>) -> Self {
        let vertex_buffer =
            SmartBuffer::from_capacity(0, gpu_tools.device(), None, BufferUsages::VERTEX | BufferUsages::COPY_DST);
        let projection_buffer = gpu_tools.device().create_buffer(&BufferDescriptor {
            label: Some("UI Projection Buffer"),
            size: size_of::<[[f32; 4]; 4]>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        Self {
            gpu_tools,
            vertex_buffer,
            projection_buffer,
        }
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        self.vertex_buffer.buffer()
    }

    pub fn proj_buffer(&self) -> &Buffer {
        &self.projection_buffer
    }

    pub fn update_proj(&self, width: u32, height: u32) {
        let (w, h) = (width as f32, height as f32);
        let proj: [[f32; 4]; 4] = ortho(0.0, w, h, 0.0, -1.0, 1.0).into();
        self.gpu_tools
            .queue()
            .write_buffer(&self.projection_buffer, 0, cast_slice(&proj));
    }

    pub fn update_vertices(&mut self, data: &[u8]) {
        let device = self.gpu_tools.device();
        let queue = self.gpu_tools.queue();
        self.vertex_buffer.update(device, queue, data);
    }

    pub fn render(
        &self,
        pass: &mut RenderPass,
        pipeline: &RenderPipeline,
        textures_bind_group: &BindGroup,
        ui_uniform_bind_group: &BindGroup,
    ) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, textures_bind_group, &[]);
        pass.set_bind_group(1, ui_uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));
        let vertex_count = self.vertex_buffer.length() / size_of::<UiVertex>() as u32;
        pass.draw(0..vertex_count, 0..1);
    }
}
