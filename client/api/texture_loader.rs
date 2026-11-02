use anyhow::Error;
use game::inventory::item_manager::ItemId;
use game::world::data::block::BlockId;

use crate::systems::texture_registry::TextureRegistry;
use engine::gpu::textures::{id::TextureId, manager::TextureManager};
use engine::render::modes::RenderMode;

pub struct TextureLoader<'a> {
    manager: &'a mut TextureManager,
    registry: &'a mut TextureRegistry,
}

impl<'a> TextureLoader<'a> {
    pub(crate) const fn new(manager: &'a mut TextureManager, registry: &'a mut TextureRegistry) -> Self {
        Self { manager, registry }
    }

    pub fn register_item(&mut self, item_id: &ItemId, path: String, render_mode: RenderMode) -> Result<TextureId, Error> {
        self.registry.register_item(item_id, self.manager, path, render_mode)
    }

    pub fn register_block(&mut self, block_id: &BlockId, path: String, render_mode: RenderMode) -> Result<TextureId, Error> {
        self.registry.register_block(block_id, self.manager, path, render_mode)
    }
}
