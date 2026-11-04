use engine::render::ui::widgets::WidgetTransform;

use crate::ui::layouts::UiLayout;

#[derive(Debug)]
pub struct SlotLayout {
    pub transform: WidgetTransform,
    pub icon: WidgetTransform,
}

impl UiLayout for SlotLayout {}
