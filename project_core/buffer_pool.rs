use std::sync::{Mutex, MutexGuard};

/// Taille maximale (nombre d'éléments) d'un buffer réintégré au pool.
///
/// Un buffer plus large que cela n'est pas conservé : il est libéré, car
/// garder un unique buffer surdimensionné figerait cette mémoire pour toute
/// la durée de vie du pool.
const MAX_KEPT_CAPACITY: usize = 1024 * 256;

pub struct BufferPool<T> {
    buffers: Mutex<Vec<Vec<T>>>,
    dafault_capacity: usize,
    /// Nombre maximum de buffers conservés simultanément.
    max_buffers: usize,
}

impl<T> BufferPool<T> {
    pub const fn new(default_capacity: usize) -> Self {
        Self {
            buffers: Mutex::new(Vec::new()),
            dafault_capacity: default_capacity,
            max_buffers: 16,
        }
    }

    /// Crée un pool dont le nombre de buffers conservés est borné.
    pub const fn with_max_buffers(default_capacity: usize, max_buffers: usize) -> Self {
        Self {
            buffers: Mutex::new(Vec::new()),
            dafault_capacity: default_capacity,
            max_buffers,
        }
    }

    fn lock_buffers(&self) -> MutexGuard<'_, Vec<Vec<T>>> {
        self.buffers.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get_buffer(&self) -> Vec<T> {
        self.lock_buffers()
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.dafault_capacity))
    }

    pub fn release_buffer(&self, mut buffer: Vec<T>) {
        buffer.clear();

        // On ne garde pas les buffers surdimensionnés : ils engloutiraient
        // de la mémoire sans jamais resservir à leur taille maximale.
        if buffer.capacity() > MAX_KEPT_CAPACITY {
            return;
        }

        let mut buffers = self.lock_buffers();
        if buffers.len() < self.max_buffers {
            buffers.push(buffer);
        }
    }
}
