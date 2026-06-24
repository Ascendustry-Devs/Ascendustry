use std::{
    collections::HashSet,
    mem::size_of,
    sync::{Arc, RwLock},
};

use crate::{geometry::vertex::Vertex, gpu::allocator::entry::EntryId};
use bytemuck::cast_slice;
use rustc_hash::{FxBuildHasher, FxHashSet};
use wgpu::{wgt::DrawIndirectArgs, BufferUsages, CommandEncoder};

use crate::gpu::{allocator::gpu_allocator::GpuAllocator, smart_buffer::SmartBuffer, tools::GpuTools};

pub struct RenderManager {
    pub gpu_tools: Arc<GpuTools>,
    pub world_buffer: Arc<RwLock<GpuAllocator>>,
    pub indirect_buffer: SmartBuffer,
    pub indirect_commands: Vec<DrawIndirectArgs>,
    pub ids_to_render: FxHashSet<EntryId>,
}

impl RenderManager {
    pub fn new(gpu_tools: Arc<GpuTools>, frame_encoder: Arc<RwLock<CommandEncoder>>) -> Self {
        let device = gpu_tools.device();
        let usages = BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC;
        let indirect_buffer = SmartBuffer::from_capacity(0, device, None, usages);

        let world_buffer = Arc::new(RwLock::new(GpuAllocator::new(Arc::clone(&gpu_tools), frame_encoder)));
        let indirect_commands = Vec::with_capacity(64);
        let ids_to_render = HashSet::with_capacity_and_hasher(128, FxBuildHasher);

        Self {
            gpu_tools,
            world_buffer,
            indirect_buffer,
            indirect_commands,
            ids_to_render,
        }
    }

    pub fn get_meshes_to_render(&self) -> Vec<EntryId> {
        self.world_buffer
            .read()
            .unwrap()
            .data
            .iter()
            .filter_map(|x| {
                if self.ids_to_render.contains(&x.id) {
                    Some(x.id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn mark_mesh_for_rendering(&mut self, id: EntryId) {
        self.ids_to_render.insert(id);
    }

    pub fn mark_meshes_for_rendering(&mut self, ids: &FxHashSet<EntryId>) {
        self.ids_to_render.extend(ids);
    }

    pub fn replace_rendering_queue(&mut self, ids: FxHashSet<EntryId>) {
        self.ids_to_render = ids;
    }

    pub fn clear_render_queue(&mut self) {
        self.ids_to_render.clear();
    }

    pub fn update_indirect_buffer(&mut self) {
        let device = self.gpu_tools.device();
        let queue = self.gpu_tools.queue();
        const VERTEX_SIZE: usize = size_of::<Vertex>();

        self.indirect_commands.clear();

        let alloc = self.world_buffer.read().unwrap();

        for entry in alloc.iter_entries_by_intersection(&self.ids_to_render) {
            self.indirect_commands.push(DrawIndirectArgs {
                vertex_count: (entry.length / VERTEX_SIZE) as u32,
                instance_count: 1,
                first_vertex: (entry.position / VERTEX_SIZE) as u32,
                first_instance: 0,
            });
        }

        if self.indirect_commands.is_empty() {
            return;
        }

        let data = cast_slice(&self.indirect_commands);
        self.indirect_buffer.update(device, queue, data);
    }
}
