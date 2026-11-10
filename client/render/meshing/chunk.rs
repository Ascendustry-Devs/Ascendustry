use bytemuck::cast_slice;
use engine::{
    geometry::vertex::Vertex,
    gpu::allocator::{entry::EntryId, error::AllocError, gpu_allocator::GpuAllocator},
};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ChunkMesh {
    pub id: Option<EntryId>,
    dirty: AtomicBool,
}

impl Default for ChunkMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkMesh {
    pub const fn new() -> Self {
        Self {
            id: None,
            dirty: AtomicBool::new(true),
        }
    }

    #[inline(always)]
    pub fn get_debug_infos(&self) -> (Option<u32>, bool) {
        (self.id, self.dirty.load(Ordering::Relaxed))
    }

    /// Updates a ChunkMesh with the data processed by [GreedyMeshingProcessor]'s threads.
    pub fn update(&mut self, vertices: &[Vertex], mesh_manager: &mut GpuAllocator) -> Result<(), AllocError> {
        // It's no longer dirty since we're updating it.
        self.dirty.store(false, Ordering::Relaxed);

        // Empty data, we free the mesh if it exists.
        if vertices.is_empty() {
            return match self.id {
                Some(mesh_id) => {
                    self.id = None;
                    mesh_manager.free(mesh_id)
                }
                None => Ok(()),
            };
        }

        let data = cast_slice(vertices);

        match self.id {
            // Mesh already exists.
            Some(mesh_id) => mesh_manager.update(mesh_id, data),
            // Creates mesh with new data and associates it with the chunk mesh.
            None => {
                let mesh_id = mesh_manager.add(data)?;
                self.id = Some(mesh_id);
                Ok(())
            }
        }
    }
}
