use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use satiscore::geometry::vertex::Vertex;
use wgpu::{wgt::DrawIndirectArgs, BufferUsages, CommandEncoder};

use crate::{
    gpu::tools::GpuTools,
    render::{
        mesh::manager::{MeshEntry, MeshId, MeshManager},
        utils::smart_buffer::SmartBuffer,
    },
};

pub struct RenderManager {
    pub gpu_tools: Arc<GpuTools>,
    pub mesh_manager: MeshManager,
    pub indirect_buffer: SmartBuffer,
    pub indirect_commands: Vec<DrawIndirectArgs>,
    pub count_buffer: SmartBuffer,
    pub ids_to_render: HashSet<MeshId>,
}

impl RenderManager {
    pub fn new(gpu_tools: Arc<GpuTools>, frame_encoder: Arc<RwLock<CommandEncoder>>) -> Self {
        let device = gpu_tools.device();
        let usages = BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC;
        let indirect_buffer = SmartBuffer::from_capacity(0, device, None, usages);
        let count_buffer = SmartBuffer::from_capacity(4, device, None, usages);

        let mesh_manager = MeshManager::new(Arc::clone(&gpu_tools), frame_encoder);
        let indirect_commands = Vec::with_capacity(64);
        let ids_to_render = HashSet::with_capacity(128);

        Self {
            gpu_tools,
            mesh_manager,
            indirect_buffer,
            indirect_commands,
            count_buffer,
            ids_to_render,
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

    pub fn update_indirect_buffer(&mut self) {
        let device = self.gpu_tools.device();
        let queue = self.gpu_tools.queue();
        const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();

        self.indirect_commands.clear();

        for m in &self.mesh_manager.data {
            if !self.ids_to_render.contains(&m.id) {
                continue;
            }
            self.indirect_commands.push(DrawIndirectArgs {
                vertex_count: (m.length / VERTEX_SIZE) as u32,
                instance_count: 1,
                first_vertex: (m.position / VERTEX_SIZE) as u32,
                first_instance: 0,
            });
        }

        if self.indirect_commands.is_empty() {
            return;
        }

        let count = self.indirect_commands.len();
        let count_raw = bytemuck::bytes_of(&count);
        self.count_buffer.update(device, queue, count_raw);

        let data = bytemuck::cast_slice(&self.indirect_commands);
        self.indirect_buffer.update(device, queue, data);
    }
}
