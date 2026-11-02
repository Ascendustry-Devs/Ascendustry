use engine::render::ui::widgets::WidgetTransform;

use crate::ui::layouts::UiLayout;

#[derive(Debug)]
pub struct SlotLayout {
    pub transform: WidgetTransform,
    pub icon: WidgetTransform,
}

impl UiLayout for SlotLayout {}

#[derive(Debug)]
pub struct HotbarLayout {
    pub transform: WidgetTransform,
    pub slots: Vec<SlotLayout>,
}

impl UiLayout for HotbarLayout {}
