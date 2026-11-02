use engine::gpu::textures::id::TextureId;

use crate::ui::adapters::UiView;

#[derive(Debug, Clone)]
pub struct SlotView {
    pub texture: TextureId,
    pub quantity: u32,
}

#[derive(Debug, Clone)]
pub struct HotbarView {
    pub slots: Vec<Option<SlotView>>,
    pub selected: usize,
}

impl UiView for HotbarView {}
