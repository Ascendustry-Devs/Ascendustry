use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: u32,
    tex_layer: u32,
    ao: f32,
    u: f32,
    v: f32,
}

impl Vertex {
    pub fn new(x: f32, y: f32, z: f32, tex_layer: u32, ao: f32, u: f32, v: f32) -> Vertex {
        return Vertex {
            position: [x, y, z],
            color: 4294967295,
            tex_layer: tex_layer,
            ao: ao,
            u,
            v,
        };
    }

    pub fn new_with_rgba(x: f32, y: f32, z: f32, r: u8, g: u8, b: u8, a: u8, tex_layer: u32, ao: f32, u: f32, v: f32) -> Vertex {
        return Vertex {
            position: [x, y, z],
            color: (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32),
            tex_layer: tex_layer,
            ao: ao,
            u,
            v,
        };
    }

    pub fn new_with_color(x: f32, y: f32, z: f32, color: u32, tex_layer: u32, ao: f32, u: f32, v: f32) -> Vertex {
        return Vertex {
            position: [x, y, z],
            color: color,
            tex_layer: tex_layer,
            ao: ao,
            u,
            v,
        };
    }

    pub fn buffer_layout() -> VertexBufferLayout<'static> {
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
