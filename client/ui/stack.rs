use std::slice::Iter;

use crate::ui::layers::DynUiLayer;

#[derive(Default)]
pub struct UiStack {
    internal: Vec<Box<dyn DynUiLayer>>,
}

impl UiStack {
    pub fn iter(&self) -> Iter<'_, Box<dyn DynUiLayer>> {
        self.internal.iter()
    }

    pub fn peek(&self) -> Option<&Box<dyn DynUiLayer>> {
        self.internal.last()
    }

    pub fn push(&mut self, layout: Box<dyn DynUiLayer>) {
        self.internal.push(layout);
    }

    pub fn pop(&mut self) {
        self.internal.pop();
    }
}
