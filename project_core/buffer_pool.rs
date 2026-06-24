use std::sync::Mutex;

pub struct BufferPool<T> {
    buffers: Mutex<Vec<Vec<T>>>,
    default_capacity: usize,
}

impl<T> BufferPool<T> {
    pub const fn new(default_capacity: usize) -> Self {
        Self {
            buffers: Mutex::new(Vec::new()),
            default_capacity,
        }
    }

    pub fn get_buffer(&self) -> Vec<T> {
        self.buffers
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.default_capacity))
    }

    pub fn release_buffer(&self, mut buffer: Vec<T>) {
        buffer.clear();
        self.buffers.lock().unwrap().push(buffer);
    }
}
