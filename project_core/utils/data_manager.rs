use std::{collections::hash_map::Iter, mem::take};

use rustc_hash::{FxBuildHasher, FxHashMap};

/// Unique id, based on the [u32] type.
///
/// Used with [DataManager].
pub type Id = u32;

/// A simple yet effective struct to store data using unique ids.
///
/// Every operation within this struct is ~O(1).
pub struct DataManager<T> {
    data: FxHashMap<Id, T>,
    free: Vec<Id>,
    next_id: Id,
}

impl<T> Default for DataManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> DataManager<T> {
    /// Makes a blank [DataManager].
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            data: FxHashMap::with_hasher(FxBuildHasher),
            free: Vec::with_capacity(8),
            next_id: 0,
        }
    }

    /// Makes a blank [DataManager] and reserves capacity.
    ///
    /// By default, data has a capacity of 0 and free has a capacity of 8.
    #[inline(always)]
    pub fn with_capacity(data_capacity: usize, free_capacity: usize) -> Self {
        Self {
            data: FxHashMap::with_capacity_and_hasher(data_capacity, FxBuildHasher),
            free: Vec::with_capacity(free_capacity),
            next_id: 0,
        }
    }

    /// Stores `data` and returns the id associated with.
    ///
    /// # Determinism
    ///
    /// This method will always return a `Some` value.
    ///
    /// # Advice
    ///
    /// Store the id wrapped with [Option] as it is required for other primitives.
    pub fn add(&mut self, data: T) -> Option<Id> {
        // Recycles old id
        if let Some(free_id) = self.free.pop() {
            self.data.insert(free_id, data);
            Some(free_id)
        }
        // Creates a new id
        else {
            let id = self.next_id;
            self.next_id += 1;
            Some(id)
        }
    }

    /// Get the data associated with `id`, if any.
    pub fn get(&self, id: Option<&Id>) -> Option<&T> {
        if let Some(id) = id {
            return self.data.get(id);
        }
        None
    }

    /// Checks if `id` is associated with data.
    pub fn exists(&self, id: Option<&Id>) -> bool {
        if let Some(id) = id {
            return self.data.contains_key(id);
        }
        false
    }

    /// Updates the data associated with `id`, if any.
    ///
    /// Returns if the update succeeded.
    pub fn update(&mut self, id: Option<&Id>, data: T) -> bool {
        if let Some(id) = id {
            if let Some(entry) = self.data.get_mut(id) {
                *entry = data;
                return true;
            }
        }
        false
    }

    pub fn iter(&self) -> Iter<'_, Id, T> {
        self.data.iter()
    }

    /// Frees and put `id` to recycle.
    ///
    /// Returns the data associated with `id`, if any.
    ///
    /// # Safety
    ///
    /// The provided id is set to `None` for safety reasons, notably to avoid accessing future stored data using a deprecated id.
    pub fn free(&mut self, id: &mut Option<Id>) -> Option<T> {
        if let Some(raw_id) = take(id) {
            return self.data.remove(&raw_id);
        };
        None
    }
}
