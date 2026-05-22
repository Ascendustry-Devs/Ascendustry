use satiscore::geometry::vertex::Vertex;
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode};

pub struct BufferLayouts;

impl BufferLayouts {
    pub const fn vertex() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: size_of::<[f32; 3]>() as BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
                VertexAttribute {
                    offset: (size_of::<[f32; 3]>() + size_of::<u32>()) as BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
                VertexAttribute {
                    offset: (size_of::<[f32; 3]>() + size_of::<u32>() * 2) as BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                VertexAttribute {
                    offset: (size_of::<[f32; 3]>() + size_of::<u32>() * 2 + size_of::<f32>()) as BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                VertexAttribute {
                    offset: (size_of::<[f32; 3]>() + size_of::<u32>() * 2 + size_of::<f32>() * 2) as BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}
