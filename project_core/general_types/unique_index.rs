use std::{collections::HashMap, hash::Hash};

#[derive(Debug)]
pub struct UniqueIndex<A, B> {
    forward: HashMap<A, B>,
    backward: HashMap<B, A>,
}

impl<A, B> UniqueIndex<A, B>
where
    A: Eq + Hash + Clone,
    B: Eq + Hash + Clone,
{
    /// Adds the entry `key` : `value` or updates the current one, if any.
    pub fn add_entry(&mut self, key: A, value: B) {
        self.forward.insert(key.clone(), value.clone());
        self.backward.insert(value, key);
    }
}

impl<A, B> UniqueIndex<A, B>
where
    A: Eq + Hash,
    B: Eq + Hash,
{
    /// Removes the entry `key` and returns an `Option` containing the associated `value`.
    pub fn remove_key(&mut self, key: &A) -> Option<B> {
        self.forward.remove(key).inspect(|value| {
            self.backward.remove(value);
        })
    }

    /// Removes the entry `value` and returns an `Option` containing the associated `key`.
    pub fn remove_value(&mut self, value: &B) -> Option<A> {
        self.backward.remove(value).inspect(|key| {
            self.forward.remove(key);
        })
    }

    /// Gets the `values` associated with `key`.
    pub fn get_key_value(&self, key: &A) -> Option<&B> {
        self.forward.get(key)
    }

    /// Gets the `keys` associated with `value`.
    pub fn get_value_key(&self, value: &B) -> Option<&A> {
        self.backward.get(value)
    }

    /// Gets the amount of `entries`.
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    /// Returns whether or not `key` is present.
    pub fn contains_key(&self, key: &A) -> bool {
        self.forward.contains_key(key)
    }

    /// Returns whether or not `value` is present.
    pub fn contains_value(&self, value: &B) -> bool {
        self.backward.contains_key(value)
    }
}

impl<A, B> Default for UniqueIndex<A, B>
where
    A: Eq + Hash,
    B: Eq + Hash,
{
    fn default() -> Self {
        Self {
            forward: HashMap::new(),
            backward: HashMap::new(),
        }
    }
}
