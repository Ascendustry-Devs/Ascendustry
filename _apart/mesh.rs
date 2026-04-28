use bytemuck::cast_slice;
use wgpu::{Buffer, BufferUsages, Device, IndexFormat, Queue};

use crate::{common::geometry::vertex::Vertex, engine::render::utils::smart_buffer::SmartBuffer};

pub struct Mesh {
    vertices: SmartBuffer,
    indices: Option<SmartBuffer>,
}

pub struct MeshData {
    vertices: (Vec<Vertex>, u32),
    indices: Option<(Vec<u32>, IndexFormat, u32)>,
}

impl Mesh {
    pub fn new(device: &Device, queue: &Queue, data: MeshData) -> Self {
        let vertices = SmartBuffer::from_data(
            cast_slice(data.get_vertex_data()),
            device,
            queue,
            None,
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
        );

        let indices = if data.has_index_data() {
            Some(SmartBuffer::from_data(
                cast_slice(data.get_index_data()),
                device,
                queue,
                Some(data.get_index_format()),
                BufferUsages::INDEX | BufferUsages::COPY_DST,
            ))
        } else {
            None
        };

        Self {
            vertices: vertices,
            indices: indices,
        }
    }

    pub fn get_vertex_buffer(&self) -> &Buffer {
        return self.vertices.buffer();
    }

    pub fn get_vertex_count(&self) -> u32 {
        return self.vertices.length() / std::mem::size_of::<Vertex>() as u32;
    }

    pub fn get_vertex_capacity(&self) -> u32 {
        return self.vertices.capacity() / std::mem::size_of::<Vertex>() as u32;
    }

    pub fn has_index_buffer(&self) -> bool {
        return self.indices.is_some();
    }

    pub fn get_index_buffer(&self) -> &Buffer {
        return &self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index buffer of a mesh but its value is None.\nMaybe the mesh is not indexed?")
            .buffer();
    }

    pub fn get_index_format(&self) -> IndexFormat {
        return self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index format of a mesh's index buffer but the buffer's value is None.\nMaybe the mesh is not indexed?")
            .format()
            .expect("Error:\ntry to get index format of a mesh's index buffer but its value is None.\nMaybe the index buffer is not correctly configured?");
    }

    pub fn get_index_count(&self) -> u32 {
        return self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index count of a mesh but its value is None.\nMaybe the mesh is not indexed?")
            .length()
            / std::mem::size_of::<u32>() as u32;
    }

    pub fn get_index_capacity(&self) -> u32 {
        return self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index capacity of a mesh but its value is None.\nMaybe the mesh is not indexed?")
            .capacity()
            / std::mem::size_of::<u32>() as u32;
    }

    pub fn update(&mut self, device: &Device, queue: &Queue, data: MeshData) {
        self.vertices.update(device, queue, cast_slice(data.get_vertex_data()));
        if data.has_index_data() {
            self.indices
                .as_mut()
                .unwrap()
                .update(device, queue, cast_slice(data.get_index_data()));
        }
    }
}

impl MeshData {
    pub fn new(vertices: Vec<Vertex>, indices: Option<Vec<u32>>) -> Self {
        let vertices = {
            let len = vertices.len() as u32;
            (vertices, len)
        };
        let indices = if let Some(indices) = indices {
            let len = indices.len() as u32;
            Some((indices, IndexFormat::Uint32, len))
        } else {
            None
        };

        Self { vertices, indices }
    }

    pub fn get_vertex_data(&self) -> &Vec<Vertex> {
        return &self.vertices.0;
    }

    pub fn get_vertex_count(&self) -> u32 {
        return self.vertices.1;
    }

    pub fn has_index_data(&self) -> bool {
        return self.indices.is_some();
    }

    pub fn get_index_data(&self) -> &Vec<u32> {
        return &self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index data of a mesh data but its value is None.\nMaybe the mesh data is not indexed?")
            .0;
    }

    pub fn get_index_format(&self) -> IndexFormat {
        return self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index format of a mesh's index buffer but its value is None.\nMaybe the mesh data is not indexed?")
            .1;
    }

    pub fn get_index_count(&self) -> u32 {
        return self
            .indices
            .as_ref()
            .expect("Error:\ntry to get index count of a mesh data but its value is None.\nMaybe the mesh data is not indexed?")
            .2;
    }
}
