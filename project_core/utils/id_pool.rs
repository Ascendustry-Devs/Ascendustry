pub struct IdPool {
    next: u32,
    free: Vec<u32>,
}

impl Default for IdPool {
    fn default() -> Self {
        Self::new()
    }
}

impl IdPool {
    pub const fn new() -> Self {
        Self {
            next: 0,
            free: Vec::new(),
        }
    }

    pub fn get_new_id(&mut self) -> u32 {
        self.free.pop().unwrap_or_else(|| {
            let id = self.next;
            self.next += 1;
            id
        })
    }

    pub fn free_id(&mut self, id: u32) {
        self.free.push(id);
    }

    pub const fn free_ids_len(&self) -> usize {
        self.free.len()
    }

    pub const fn capacity(&self) -> usize {
        self.free.capacity()
    }
}
