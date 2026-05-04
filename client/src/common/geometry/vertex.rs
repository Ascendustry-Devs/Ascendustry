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

    pub fn copy_with_pos(&self, x: f32, y: f32, z: f32) -> Self {
        let mut copy = self.clone();
        copy.position = [x, y, z];
        copy
    }

    pub fn player_vertex(pos: (f32, f32, f32), u: f32, v: f32) -> Vertex {
        let (x, y, z) = pos;
        Vertex {
            position: [x, y, z],
            color: 0x00FFFFFF,
            tex_layer: 1,
            ao: 3.0,
            u,
            v,
        }
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

pub fn generate_cube(x: f32, y: f32, z: f32) -> Vec<Vertex> {
    let h = 0.5; // Cube de 1x1x1 centrée sur (x,y,z)

    // 8 coins du cube
    let p0 = (x - h, y - h, z - h); // -x,-y,-z
    let p1 = (x - h, y - h, z + h); // -x,-y,+z
    let p2 = (x - h, y + h, z + h); // -x,+y,+z
    let p3 = (x - h, y + h, z - h); // -x,+y,-z
    let p4 = (x + h, y - h, z + h); // +x,-y,+z
    let p5 = (x + h, y - h, z - h); // +x,-y,-z
    let p6 = (x + h, y + h, z - h); // +x,+y,-z
    let p7 = (x + h, y + h, z + h); // +x,+y,+z

    let mut v = Vec::with_capacity(36);

    // -X (gauche)
    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p1, 1.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 1.0),
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 1.0),
    ]);

    // +X (droite)
    v.extend_from_slice(&[
        Vertex::player_vertex(p4, 0.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 0.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p4, 0.0, 0.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p7, 0.0, 1.0),
    ]);

    // -Y (bas)
    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 0.0),
        Vertex::player_vertex(p1, 1.0, 1.0),
        Vertex::player_vertex(p5, 0.0, 1.0),
        Vertex::player_vertex(p4, 1.0, 1.0),
        Vertex::player_vertex(p1, 0.0, 1.0),
    ]);

    // +Y (haut)
    v.extend_from_slice(&[
        Vertex::player_vertex(p3, 0.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p6, 0.0, 1.0),
    ]);

    // -Z (arrière)
    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p3, 1.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 1.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p5, 0.0, 1.0),
    ]);

    // +Z (avant)
    v.extend_from_slice(&[
        Vertex::player_vertex(p1, 0.0, 0.0),
        Vertex::player_vertex(p4, 1.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p1, 0.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p2, 0.0, 1.0),
    ]);

    v
}