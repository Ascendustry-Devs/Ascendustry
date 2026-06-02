/// A simple yet effective struct to get track of the last 2 values used.
#[derive(Clone)]
pub struct Updatable<T: Clone + PartialEq> {
    last: T,
    current: T,
}

impl<T: Clone + PartialEq> Updatable<T> {
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
    pub fn update_eq<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> T,
    {
        self.update(f(&self.current));
    }

    #[inline(always)]
    pub fn last(&self) -> &T {
        &self.last
    }

    // #[inline(always)]
    // pub fn last_mut(&mut self) -> &mut T {
    //     &mut self.last
    // }

    #[inline(always)]
    pub fn current(&self) -> &T {
        &self.current
    }

    #[inline(always)]
    pub fn current_mut(&mut self) -> &mut T {
        &mut self.current
    }

    #[inline(always)]
    pub fn values_mut(&mut self) -> (&mut T, &mut T) {
        (&mut self.current, &mut self.last)
    }

    #[inline(always)]
    pub fn has_changed(&self) -> bool {
        self.current.ne(&self.last)
    }

    #[inline(always)]
    pub fn change(&self) -> Option<&T> {
        if self.has_changed() {
            Some(&self.current)
        } else {
            None
        }
    }
}

impl<T: Copy + PartialEq> Updatable<T> {
    #[inline(always)]
    pub fn update_by_copy(&mut self, value: T) {
        let old = self.current;
        self.current = value;
        self.last = old;
    }
}
