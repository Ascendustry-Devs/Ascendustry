use anyhow::Error;

use engine::{
    gpu::textures::{id::TextureID, manager::TextureManager},
    render::modes::RenderMode,
};

pub struct TextureRegistry;

impl TextureRegistry {
    pub fn register(texture_manager: &mut TextureManager, path: String, render_mode: RenderMode) -> Result<TextureID, Error> {
        let Ok(texture) = image::open(path) else {
            return Err(Error::msg("idk"));
        };

        texture_manager.register_array(render_mode, texture.as_bytes())
    }
}
