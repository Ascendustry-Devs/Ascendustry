pub type EntryId = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct AllocEntry {
    pub id: EntryId,
    pub position: usize,
    pub length: usize,
}

impl AllocEntry {
    pub fn new(id: EntryId, position: usize, length: usize) -> Self {
        Self { id, position, length }
    }
}
