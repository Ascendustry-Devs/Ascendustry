/// A simple yet effective struct to get track of the last 2 values used.
pub struct Updatable<T: Clone> {
    last: T,
    current: T,
}

impl<T: Clone> Updatable<T> {
    #[inline(always)]
    pub fn new(value: T) -> Self {
        Self {
            current: value.clone(),
            last: value,
        }
    }

    #[inline(always)]
    pub fn update(&mut self, value: T) {
        self.last = std::mem::replace(&mut self.current, value);
    }

    #[inline(always)]
    pub fn last(&self) -> &T {
        &self.last
    }

    #[inline(always)]
    pub fn current(&self) -> &T {
        &self.current
    }

    #[inline(always)]
    pub fn current_mut(&mut self) -> &mut T {
        &mut self.current
    }
}

impl<T: Clone + PartialEq> Updatable<T> {
    #[inline(always)]
    pub fn has_changed(&self) -> bool {
        self.current != self.last
    }
}
