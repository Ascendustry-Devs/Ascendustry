use std::slice::IterMut;

use crate::ui::layers::DynUiLayer;

pub struct UiStack {
    internal: Vec<Box<dyn DynUiLayer>>,
}

impl UiStack {
    pub const fn new() -> Self {
        Self { internal: Vec::new() }
    }

    pub fn peek(&self) -> Option<&dyn DynUiLayer> {
        self.internal.last().map(|layer| layer.as_ref())
    }

    pub fn push(&mut self, layout: Box<dyn DynUiLayer>) {
        self.internal.push(layout);
    }

    pub fn pop(&mut self) -> Option<Box<dyn DynUiLayer>> {
        self.internal.pop()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Box<dyn DynUiLayer>> {
        self.internal.iter_mut()
    }
}
