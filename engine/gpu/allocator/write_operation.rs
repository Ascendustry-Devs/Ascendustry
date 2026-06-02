use crate::gpu::allocator::gpu_allocator::MeshId;

pub struct WriteOperation {
    pub mesh_id: MeshId,
    pub offset: usize,
    pub len: usize,
    pub arena_offset: usize,
}
