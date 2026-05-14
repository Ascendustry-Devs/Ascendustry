use std::collections::HashSet;
use std::mem;

use wgpu::{wgt::DrawIndirectArgs, BufferUsages, Device, Queue};

use crate::{
    common::geometry::vertex::Vertex,
    engine::render::{
        mesh::manager::{MeshEntry, MeshId, MeshManager},
        utils::smart_buffer::SmartBuffer,
    },
};

pub struct RenderManager {
    pub mesh_manager: MeshManager,
    pub indirect_buffer: SmartBuffer,
    pub indirect_commands: Vec<DrawIndirectArgs>,
    pub count_buffer: SmartBuffer,
    pub ids_to_render: HashSet<MeshId>,
}

impl RenderManager {
    pub fn new(device: &Device) -> Self {
        Self {
            mesh_manager: MeshManager::new(device),
            indirect_buffer: SmartBuffer::from_capacity(
                0,
                device,
                None,
                BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            ),
            indirect_commands: vec![],
            count_buffer: SmartBuffer::from_capacity(
                4,
                device,
                None,
                BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            ),
            ids_to_render: HashSet::new(),
        }
    }

    pub fn get_meshes_to_render(&self) -> Vec<&MeshEntry> {
        self.mesh_manager
            .data
            .iter()
            .filter_map(|x| if self.ids_to_render.contains(&x.id) { Some(x) } else { None })
            .collect()
    }

    pub fn mark_mesh_for_rendering(&mut self, id: MeshId) {
        self.ids_to_render.insert(id);
    }

    pub fn mark_meshes_for_rendering(&mut self, ids: &HashSet<MeshId>) {
        self.ids_to_render.extend(ids);
    }

    pub fn replace_rendering_queue(&mut self, ids: HashSet<MeshId>) {
        self.ids_to_render = ids;
    }

    pub fn clear_render_queue(&mut self) {
        self.ids_to_render.clear();
    }

    pub fn update_indirect_buffer(&mut self, device: &Device, queue: &Queue) {
        let mesh_regions: Vec<(usize, usize)> = self
            .mesh_manager
            .data
            .iter()
            .filter(|m| self.ids_to_render.contains(&m.id))
            .map(|m| (m.position, m.length))
            .collect();

        if mesh_regions.is_empty() {
            self.indirect_commands.clear();
            return;
        }

        let mut commands: Vec<DrawIndirectArgs> = Vec::with_capacity(mesh_regions.len());
        let vertex_size = std::mem::size_of::<Vertex>() as u32;

        for (offset, length) in &mesh_regions {
            let vertex_count = length / vertex_size as usize;
            commands.push(DrawIndirectArgs {
                vertex_count: vertex_count as u32,
                instance_count: 1,
                first_vertex: (offset / vertex_size as usize) as u32,
                first_instance: 0,
            });
        }

        self.count_buffer
            .update(device, queue, bytemuck::bytes_of(&(mesh_regions.len() as u32)));

        let data = bytemuck::cast_slice(&commands);
        self.indirect_buffer.update(device, queue, data);
        self.indirect_commands = commands;
    }
}
