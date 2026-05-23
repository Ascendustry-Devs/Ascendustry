use std::{
    collections::{vec_deque::Iter, HashSet, VecDeque},
    hash::{BuildHasher, Hash, RandomState},
};

use rustc_hash::FxBuildHasher;

/// A FIFO data structure whose main characteristic is that each element is guaranteed to be **unique**.
///
/// ## Pros
///
/// - really fast (every operation is ~O(1))
/// - CPU cache friendly
/// - growable
///
/// ## Cons
///
/// - consumes more memory
/// - single-entry queue
/// - limited number of operation
pub struct UniqueQueue<T, S = RandomState> {
    queue: VecDeque<T>,
    set: HashSet<T, S>,
}

pub type FxUniqueQueue<T> = UniqueQueue<T, FxBuildHasher>;

impl<T: Hash + Eq + Clone, S: BuildHasher + Default> UniqueQueue<T, S> {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            set: HashSet::with_hasher(S::default()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            set: HashSet::with_capacity_and_hasher(capacity, S::default()),
        }
    }

    pub fn with_hasher(hasher: S) -> Self {
        Self {
            queue: VecDeque::new(),
            set: HashSet::with_hasher(hasher),
        }
    }

    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            set: HashSet::with_capacity_and_hasher(capacity, hasher),
        }
    }

    pub fn push_back(&mut self, value: T) -> bool {
        if self.contains(&value) {
            return false;
        }
        self.set.insert(value.clone());
        self.queue.push_back(value);
        true
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.queue.pop_front().inspect(|value| {
            self.set.remove(value);
        })
    }

    pub fn front(&self) -> Option<&T> {
        self.queue.front()
    }

    pub fn back(&self) -> Option<&T> {
        self.queue.back()
    }

    pub fn contains(&self, value: &T) -> bool {
        self.set.contains(value)
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.set.clear();
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.queue.retain(|element| {
            if f(element) {
                true
            } else {
                self.set.remove(element);
                false
            }
        });
    }
}

impl<T: Hash + Eq + Clone, S: BuildHasher + Default> Default for UniqueQueue<T, S> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            set: Default::default(),
        }
    }
}
