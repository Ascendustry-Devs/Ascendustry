use anyhow::Error;

use engine::{
    gpu::textures::{id::TextureID, manager::TextureManager},
    render::modes::RenderMode,
};

pub struct TextureRegistry;

impl TextureRegistry {
    pub fn register(texture_manager: &mut TextureManager, path: String, render_mode: RenderMode) -> Result<TextureID, Error> {
        let texture = image::open(path).map_err(|_| Error::msg("idk"))?;
        let rgba = texture.into_rgba8();
        texture_manager.register_array(render_mode, &rgba.into_raw())
    }
}
