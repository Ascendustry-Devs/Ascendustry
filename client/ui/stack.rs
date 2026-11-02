use std::slice::IterMut;

use crate::ui::layers::DynUiLayer;

#[derive(Default)]
pub struct UiStack {
    internal: Vec<Box<dyn DynUiLayer>>,
}

impl UiStack {
    pub fn peek(&self) -> Option<&Box<dyn DynUiLayer>> {
        self.internal.last()
    }

    pub fn push(&mut self, layout: Box<dyn DynUiLayer>) {
        self.internal.push(layout);
    }

    pub fn pop(&mut self) {
        self.internal.pop();
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Box<dyn DynUiLayer>> {
        self.internal.iter_mut()
    }
}
