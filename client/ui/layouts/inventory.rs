use engine::render::ui::widgets::WidgetTransform;

use crate::ui::layouts::{slot::SlotLayout, UiLayout};

#[derive(Debug)]
pub struct InventoryLayout {
    pub transform: WidgetTransform,
    pub slots: Vec<SlotLayout>,
}

impl UiLayout for InventoryLayout {}
