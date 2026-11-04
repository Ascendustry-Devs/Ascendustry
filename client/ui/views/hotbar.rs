use crate::ui::adapters::UiView;
use crate::ui::views::slot::SlotView;

#[derive(Debug, Clone)]
pub struct HotbarView {
    pub slots: Vec<Option<SlotView>>,
    pub selected: usize,
}

impl UiView for HotbarView {}
