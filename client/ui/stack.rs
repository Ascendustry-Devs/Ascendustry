use std::slice::Iter;

use crate::ui::layouts::UiLayout;

#[derive(Default)]
pub struct UiStack {
    internal: Vec<Box<dyn UiLayout>>,
}

impl UiStack {
    pub fn iter(&self) -> Iter<'_, Box<dyn UiLayout>> {
        self.internal.iter()
    }

    pub fn peek(&self) -> Option<&Box<dyn UiLayout>> {
        self.internal.last()
    }

    pub fn push(&mut self, layout: Box<dyn UiLayout>) {
        self.internal.push(layout);
    }

    pub fn pop(&mut self) {
        self.internal.pop();
    }
}
