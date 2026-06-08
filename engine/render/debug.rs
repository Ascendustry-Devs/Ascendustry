use wgpu::{Buffer, RenderPass, RenderPipeline};

use crate::gpu::resources::pipeline::Pipelines;

pub struct DebugRenderResources {
    pub wireframe: bool,
    pub pipelines: Pipelines,
    pub gizmo_render_pipeline: RenderPipeline,
    pub gizmo_buffer: Buffer,
    pub show_chunk_borders: bool,
    pub chunk_borders_buffer: Buffer,
}

impl DebugRenderResources {
    pub fn new(
        pipelines: Pipelines,
        gizmo_render_pipeline: RenderPipeline,
        gizmo_buffer: Buffer,
        chunk_borders_buffer: Buffer,
    ) -> Self {
        Self {
            wireframe: false,
            pipelines,
            gizmo_render_pipeline,
            gizmo_buffer,
            show_chunk_borders: false,
            chunk_borders_buffer,
        }
    }

    pub fn pass(&mut self, render_pass: &mut RenderPass) {
        if self.wireframe || self.show_chunk_borders {
            render_pass.set_pipeline(&self.gizmo_render_pipeline);
            if self.wireframe {
                render_pass.set_vertex_buffer(0, self.gizmo_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
            if self.show_chunk_borders {
                render_pass.set_vertex_buffer(0, self.chunk_borders_buffer.slice(..));
                render_pass.draw(0..24, 0..1);
            }
        }
    }
}
