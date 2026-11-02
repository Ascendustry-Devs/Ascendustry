use anyhow::Error;

use engine::{
    gpu::textures::{id::TextureId, manager::TextureManager},
    render::modes::RenderMode,
};
use game::{inventory::item_manager::ItemId, world::data::block::BlockId};
use project_core::general_types::unique_index::UniqueIndex;

#[derive(Debug, Default)]
pub struct TextureRegistry {
    items: UniqueIndex<ItemId, TextureId>,
    blocks: UniqueIndex<BlockId, TextureId>,
}

impl TextureRegistry {
    pub fn get_item_texture(&self, item_id: &ItemId) -> Option<&TextureId> {
        self.items.get_key_value(item_id)
    }

    pub fn get_block_texture(&self, block_id: &BlockId) -> Option<&TextureId> {
        self.blocks.get_key_value(block_id)
    }

    pub fn register(
        &mut self,
        texture_manager: &mut TextureManager,
        path: String,
        render_mode: RenderMode,
    ) -> Result<TextureId, Error> {
        let texture = image::open(path).map_err(|_| Error::msg("idk"))?;
        let rgba = texture.into_rgba8();
        texture_manager.register_array(render_mode, &rgba.into_raw())
    }

    pub fn register_item(
        &mut self,
        item_id: &ItemId,
        texture_manager: &mut TextureManager,
        path: String,
        render_mode: RenderMode,
    ) -> Result<TextureId, Error> {
        self.register(texture_manager, path, render_mode).inspect(|texture_id| {
            self.items.add_entry(*item_id, texture_id.clone());
        })
    }

    pub fn register_block(
        &mut self,
        block_id: &BlockId,
        texture_manager: &mut TextureManager,
        path: String,
        render_mode: RenderMode,
    ) -> Result<TextureId, Error> {
        self.register(texture_manager, path, render_mode).inspect(|texture_id| {
            self.blocks.add_entry(*block_id, texture_id.clone());
        })
    }
}
