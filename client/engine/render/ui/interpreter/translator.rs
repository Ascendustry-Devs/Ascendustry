use crate::{
    gpu::textures::manager::TextureManager,
    render::ui::{geometry::ui_vertex::UiVertex, widgets::DrawCommand},
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
                let (x_min, x_max, y_min, y_max) = (x, x + w, y, y + h);
                let Some((u_min, u_max, v_min, v_max)) = texture_manager.get_ui_uvs(&texture) else {
                    panic!("UiTranslator: Texture not found: {:?}", texture);
                };

                let arg = if texture_manager.is_array_texture(&texture) {
                    texture.id() // couche de l'array
                } else {
                    0xFFFFFFFF // teinte blanche pour l'atlas
                };

                vertices.extend([
                    UiVertex::textured(x_min, y_min, u_min, v_min, arg),
                    UiVertex::textured(x_max, y_min, u_max, v_min, arg),
                    UiVertex::textured(x_min, y_max, u_min, v_max, arg),
                    UiVertex::textured(x_min, y_max, u_min, v_max, arg),
                    UiVertex::textured(x_max, y_min, u_max, v_min, arg),
                    UiVertex::textured(x_max, y_max, u_max, v_max, arg),
                ]);
            }
        }
    }
}
