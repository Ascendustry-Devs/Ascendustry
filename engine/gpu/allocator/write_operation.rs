use crate::gpu::allocator::entry::EntryId;

pub(super) struct WriteOperation {
    pub id: EntryId,
    pub len: usize,
    pub buffer_offset: usize,
    pub arena_offset: usize,
}

impl WriteOperation {
    pub const fn new(id: EntryId, len: usize, buffer_offset: usize, arena_offset: usize) -> Self {
        Self {
            id,
            len,
            buffer_offset,
            arena_offset,
        }
    }
}
