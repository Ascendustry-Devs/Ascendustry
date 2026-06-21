use crate::render::ui::widgets::{DrawCommand, Widget, WidgetTransform, WidgetType};

pub struct List {
    transform: WidgetTransform,
    children: Vec<Box<WidgetType>>,
}

impl Widget for List {
    fn transform(&self) -> &WidgetTransform {
        &self.transform
    }

    fn draw(&self, commands: &mut Vec<DrawCommand>) {
        for child in self.children.iter() {
            child.draw(commands);
        }
    }
}

impl List {
    pub fn new(transform: WidgetTransform, children: Vec<Box<WidgetType>>) -> Self {
        Self { transform, children }
    }

    pub fn children(&self) -> &Vec<Box<WidgetType>> {
        &self.children
    }
}
