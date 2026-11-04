use engine::gpu::textures::id::TextureId;

use crate::ui::adapters::UiView;

#[derive(Debug, Clone)]
pub struct SlotView {
    pub texture: TextureId,
    pub quantity: u32,
}

impl UiView for SlotView {}
