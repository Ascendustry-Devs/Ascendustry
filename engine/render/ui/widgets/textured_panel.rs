use project_core::utils::data_manager::Id;

use crate::render::ui::widgets::{DrawCommand, Widget, WidgetTransform, WidgetType};

pub struct TexturedPanel {
    transform: WidgetTransform,
    texture: Id,
    child: Option<Box<WidgetType>>,
}

impl Widget for TexturedPanel {
    fn transform(&self) -> &WidgetTransform {
        &self.transform
    }

    fn draw(&self, commands: &mut Vec<DrawCommand>) {
        commands.push(DrawCommand::TexturedPanel {
            transform: self.transform.clone(),
            texture: self.texture,
        });
        if let Some(child) = self.child.as_ref() {
            child.draw(commands);
        }
    }
}

impl TexturedPanel {
    pub const fn new(transform: WidgetTransform, texture: u32, child: Option<Box<WidgetType>>) -> Self {
        Self {
            transform,
            texture,
            child,
        }
    }
}
