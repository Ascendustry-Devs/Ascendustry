use game::world::data::chunk::CHUNK_SIZE;
use wgpu::{Buffer, RenderPass, RenderPipeline};

use crate::{geometry::vertex::Vertex, gpu::resources::pipeline::Pipelines};

const fn debug_vertex(x: i32, y: i32, z: i32, rgba: u32) -> Vertex {
    Vertex::new_with_color(x as f32, y as f32, z as f32, rgba, 0, 3.0, 0.0, 0.0)
}

const RED: u32 = 0xFF0000FF;
const GREEN: u32 = 0x0000FFFF;
const BLUE: u32 = 0x00FF00FF;
const BLACK: u32 = 0x000000FF;

pub const GIZMO: [Vertex; 6] = [
    // +X
    debug_vertex(0, 0, 0, RED),
    debug_vertex(1, 0, 0, RED),
    // +Y
    debug_vertex(0, 0, 0, GREEN),
    debug_vertex(0, 1, 0, GREEN),
    // +Z
    debug_vertex(0, 0, 0, BLUE),
    debug_vertex(0, 0, 1, BLUE),
];

const C: i32 = CHUNK_SIZE;

pub const CHUNK_BORDERS: [Vertex; 24] = [
    debug_vertex(0, 0, 0, RED),
    debug_vertex(C, 0, 0, RED),
    debug_vertex(0, 0, C, BLACK),
    debug_vertex(C, 0, C, BLACK),
    debug_vertex(0, C, 0, BLACK),
    debug_vertex(C, C, 0, BLACK),
    debug_vertex(0, C, C, BLACK),
    debug_vertex(C, C, C, BLACK),
    debug_vertex(0, 0, 0, GREEN),
    debug_vertex(0, C, 0, GREEN),
    debug_vertex(0, 0, C, BLACK),
    debug_vertex(0, C, C, BLACK),
    debug_vertex(C, 0, 0, BLACK),
    debug_vertex(C, C, 0, BLACK),
    debug_vertex(C, 0, C, BLACK),
    debug_vertex(C, C, C, BLACK),
    debug_vertex(0, 0, 0, BLUE),
    debug_vertex(0, 0, C, BLUE),
    debug_vertex(C, 0, 0, BLACK),
    debug_vertex(C, 0, C, BLACK),
    debug_vertex(0, C, 0, BLACK),
    debug_vertex(0, C, C, BLACK),
    debug_vertex(C, C, 0, BLACK),
    debug_vertex(C, C, C, BLACK),
];

pub struct DebugRenderResources {
    pub wireframe: bool,
    pub pipelines: Pipelines,
    pub gizmo_render_pipeline: RenderPipeline,
    pub gizmo_buffer: Buffer,
    pub show_chunk_borders: bool,
    pub chunk_borders_buffer: Buffer,
}

impl DebugRenderResources {
    pub const fn new(
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
        if self.show_chunk_borders {
            render_pass.set_pipeline(&self.gizmo_render_pipeline);

            render_pass.set_vertex_buffer(0, self.chunk_borders_buffer.slice(..));
            render_pass.draw(0..24, 0..1);

            render_pass.set_vertex_buffer(0, self.gizmo_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }
}
