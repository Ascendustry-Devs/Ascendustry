use crate::ui::{adapters::UiView, views::slot::SlotView};

#[derive(Debug, Clone)]
pub struct InventoryView {
    pub slots: Vec<Option<SlotView>>,
    pub selected: usize,
}

impl UiView for InventoryView {}
