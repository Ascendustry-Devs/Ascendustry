use bytemuck::{Pod, Zeroable};

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
    pub const fn new(x: f32, y: f32, z: f32, tex_layer: u32, ao: f32, u: f32, v: f32) -> Vertex {
        Vertex {
            position: [x, y, z],
            color: 4294967295,
            tex_layer,
            ao,
            u,
            v,
        }
    }

    pub fn copy_with_pos(&self, x: f32, y: f32, z: f32) -> Self {
        let mut copy = self.clone();
        copy.position = [x, y, z];
        copy
    }

    pub const fn player_vertex(pos: (f32, f32, f32), u: f32, v: f32) -> Vertex {
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

    pub const fn new_with_rgba(
        x: f32,
        y: f32,
        z: f32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        tex_layer: u32,
        ao: f32,
        u: f32,
        v: f32,
    ) -> Vertex {
        Vertex {
            position: [x, y, z],
            color: (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32),
            tex_layer,
            ao,
            u,
            v,
        }
    }

    pub const fn new_with_color(x: f32, y: f32, z: f32, color: u32, tex_layer: u32, ao: f32, u: f32, v: f32) -> Vertex {
        Vertex {
            position: [x, y, z],
            color,
            tex_layer,
            ao,
            u,
            v,
        }
    }
}

pub fn generate_cube(x: f32, y: f32, z: f32) -> Vec<Vertex> {
    let h = 0.5;

    let p0 = (x - h, y - h, z - h);
    let p1 = (x - h, y - h, z + h);
    let p2 = (x - h, y + h, z + h);
    let p3 = (x - h, y + h, z - h);
    let p4 = (x + h, y - h, z + h);
    let p5 = (x + h, y - h, z - h);
    let p6 = (x + h, y + h, z - h);
    let p7 = (x + h, y + h, z + h);

    let mut v = Vec::with_capacity(36);

    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p1, 1.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 1.0),
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 1.0),
    ]);

    v.extend_from_slice(&[
        Vertex::player_vertex(p4, 0.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 0.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p4, 0.0, 0.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p7, 0.0, 1.0),
    ]);

    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 0.0),
        Vertex::player_vertex(p1, 1.0, 1.0),
        Vertex::player_vertex(p5, 0.0, 1.0),
        Vertex::player_vertex(p4, 1.0, 1.0),
        Vertex::player_vertex(p1, 0.0, 1.0),
    ]);

    v.extend_from_slice(&[
        Vertex::player_vertex(p3, 0.0, 0.0),
        Vertex::player_vertex(p2, 1.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 0.0),
        Vertex::player_vertex(p7, 1.0, 1.0),
        Vertex::player_vertex(p6, 0.0, 1.0),
    ]);

    v.extend_from_slice(&[
        Vertex::player_vertex(p0, 0.0, 0.0),
        Vertex::player_vertex(p3, 1.0, 0.0),
        Vertex::player_vertex(p5, 1.0, 1.0),
        Vertex::player_vertex(p3, 0.0, 1.0),
        Vertex::player_vertex(p6, 1.0, 1.0),
        Vertex::player_vertex(p5, 0.0, 1.0),
    ]);

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
