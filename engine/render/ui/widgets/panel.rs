use crate::render::ui::widgets::{DrawCommand, Widget, WidgetTransform, WidgetType};

pub struct Panel {
    transform: WidgetTransform,
    color: u32,
    child: Option<Box<WidgetType>>,
}

impl Widget for Panel {
    fn transform(&self) -> &WidgetTransform {
        &self.transform
    }

    fn draw(&self, commands: &mut Vec<DrawCommand>) {
        commands.push(DrawCommand::Panel {
            transform: self.transform.clone(),
            color: self.color,
        });
        if let Some(child) = self.child.as_ref() {
            child.draw(commands);
        }
    }
}

impl Panel {
    pub fn new(transform: WidgetTransform, color: u32, child: Option<Box<WidgetType>>) -> Self {
        Self { transform, color, child }
    }
}
