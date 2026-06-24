use crate::{geometry::vertex::Vertex, render::ui::geometry::ui_vertex::UiVertex};
use wgpu::{vertex_attr_array, BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode};

pub struct BufferLayouts;

impl BufferLayouts {
    pub const fn build_vertex_layout<T>(attributes: &'static [VertexAttribute]) -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<T>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes,
        }
    }

    pub const fn vertex() -> VertexBufferLayout<'static> {
        const ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![
            0 => Float32x3, // position
            1 => Uint32,    // color
            2 => Uint32,    // tex_layer
            3 => Float32,   // ao
            4 => Float32,   // u
            5 => Float32,   // v
        ];
        Self::build_vertex_layout::<Vertex>(ATTRIBUTES)
    }

    pub const fn ui_vertex() -> VertexBufferLayout<'static> {
        const ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![
            0 => Uint32,  // x
            1 => Uint32,  // y
            2 => Float32, // u
            3 => Float32, // v
            4 => Uint32,  // color
        ];
        Self::build_vertex_layout::<UiVertex>(ATTRIBUTES)
    }
}
