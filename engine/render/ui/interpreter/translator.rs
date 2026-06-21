use crate::{
    gpu::textures::{id::TextureID, manager::TextureManager},
    render::{
        modes::RenderMode,
        ui::{geometry::ui_vertex::UiVertex, widgets::DrawCommand},
    },
};

pub struct UiTranslator;

impl UiTranslator {
    pub fn translate(commands: Vec<DrawCommand>, texture_manager: &TextureManager) -> Vec<UiVertex> {
        let mut output = Vec::with_capacity(commands.len() * 6);

        for command in commands {
            Self::process(command, texture_manager, &mut output);
        }

        output
    }

    fn process(command: DrawCommand, texture_manager: &TextureManager, vertices: &mut Vec<UiVertex>) {
        match command {
            DrawCommand::Panel { transform, color } => {
                let (x, y, w, h) = transform.extract();
                vertices.extend([
                    UiVertex::colored(x, y, color),
                    UiVertex::colored(x, y + h, color),
                    UiVertex::colored(x + w, y, color),
                    UiVertex::colored(x + w, y, color),
                    UiVertex::colored(x, y + h, color),
                    UiVertex::colored(x + w, y + h, color),
                ]);
            }
            DrawCommand::TexturedPanel { transform, texture } => {
                let (x, y, w, h) = transform.extract();
                let id = TextureID::new(RenderMode::UI, texture);
                let (u_min, u_max, v_min, v_max) = texture_manager.get_ui_uvs(&id).expect("UiTranslator: Texture not found");
                let (x_min, x_max, y_min, y_max) = (x, x + w, y, y + h);
                vertices.extend([
                    UiVertex::textured(x_min, y_min, u_min, v_min),
                    UiVertex::textured(x_max, y_min, u_max, v_min),
                    UiVertex::textured(x_min, y_max, u_min, v_max),
                    UiVertex::textured(x_min, y_max, u_min, v_max),
                    UiVertex::textured(x_max, y_min, u_max, v_min),
                    UiVertex::textured(x_max, y_max, u_max, v_max),
                ]);
            }
        }
    }
}
