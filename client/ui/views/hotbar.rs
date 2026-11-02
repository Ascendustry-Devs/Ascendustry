use crate::ui::adapters::UiView;

#[derive(Debug)]
pub struct SlotView {
    pub icon: u32,
    pub quantity: u32,
}

#[derive(Debug)]
pub struct HotbarView {
    pub slots: Vec<Option<SlotView>>,
    pub selected: usize,
}

impl UiView for HotbarView {}
