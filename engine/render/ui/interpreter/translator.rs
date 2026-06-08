use crate::render::ui::{geometry::ui_vertex::UiVertex, widgets::DrawCommand};

pub struct UiTranslator;

impl UiTranslator {
    pub fn translate(commands: Vec<DrawCommand>) -> Vec<UiVertex> {
        let mut output = Vec::with_capacity(commands.len());

        for command in commands {
            Self::process(command, &mut output);
        }

        output
    }

    fn process(command: DrawCommand, vertices: &mut Vec<UiVertex>) {
        match command {
            DrawCommand::Panel { transform, color } => {
                let (x, y, w, h) = transform.extract();
                vertices.extend([
                    UiVertex::with_no_texture(x, y, color),
                    UiVertex::with_no_texture(x, y + h, color),
                    UiVertex::with_no_texture(x + w, y, color),
                    UiVertex::with_no_texture(x + w, y, color),
                    UiVertex::with_no_texture(x, y + h, color),
                    UiVertex::with_no_texture(x + w, y + h, color),
                ]);
            }
        }
    }
}
