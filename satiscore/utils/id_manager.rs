use std::{mem, ops::Deref};

/// Unique id, based on the [usize] type.
///
/// Used with [DataManager] to store keys to unique data.
pub type Id = Option<usize>;

/// A simple yet effective struct to manage unique ids to store data.
///
/// Every operation within this struct is O(1) or close to it.
pub struct DataManager<T> {
    data: Vec<Option<T>>,
    free: Vec<Id>,
    next_id: usize,
}

impl<T> DataManager<T> {
    /// Makes a blank [DataManager].
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            free: Vec::new(),
            next_id: 0,
        }
    }

    /// Makes a blank [DataManager] with provided capacity.
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            free: Vec::new(),
            next_id: 0,
        }
    }

    /// Gets an unused id.
    // pub fn add(&mut self, data: T) -> Id {
    //     // On recupère un id existant
    //     if let Some(free_id) = self.free.pop() {
    //         let mut entry = self.data.get_mut(free_id.unwrap()).unwrap();

    //         free_id
    //     }
    //     // On crée un nouvel id
    //     else {
    //         self.
    //     }
    // }

    /// Frees an id.
    pub fn free(&mut self, id: &mut Id) -> Option<T> {
        if let Some(id) = id {
            let id = *id;
            if self.data.len() <= id {
                return None;
            }
            if let Some(entry) = self.data.get_mut(id) {
                return mem::replace(entry, None);
            }
        };

        None
    }
}
