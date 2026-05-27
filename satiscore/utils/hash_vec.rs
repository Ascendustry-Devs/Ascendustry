use std::collections::HashMap;

/// A simple yet effective struct to store like a Vec except access is O(1)
pub struct HashVec<T> {
    data: Vec<Option<T>>,
    keys: HashMap<usize, usize>,
}

impl<T> HashVec<T> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            keys: HashMap::new(),
        }
    }

    pub fn push() {}
}
