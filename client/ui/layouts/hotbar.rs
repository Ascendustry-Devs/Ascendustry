use engine::render::ui::widgets::WidgetTransform;

use crate::ui::layouts::{slot::SlotLayout, UiLayout};

#[derive(Debug)]
pub struct HotbarLayout {
    pub transform: WidgetTransform,
    pub slots: Vec<SlotLayout>,
}

impl UiLayout for HotbarLayout {}
