use anyhow::Error;

use crate::systems::texture_registry::TextureRegistry;
use engine::gpu::textures::{id::TextureID, manager::TextureManager};
use engine::render::modes::RenderMode;

pub struct TextureLoader<'a> {
    texture_manager: &'a mut TextureManager,
}

impl<'a> TextureLoader<'a> {
    pub(crate) const fn new(texture_manager: &'a mut TextureManager) -> Self {
        Self { texture_manager }
    }

    pub fn register(&mut self, path: String, render_mode: RenderMode) -> Result<TextureID, Error> {
        TextureRegistry::register(self.texture_manager, path, render_mode)
    }
}
