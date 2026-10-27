pub mod hotbar;

use engine::render::ui::widgets::WidgetTransform;

use crate::ui::action::UiAction;

pub enum UiMode {
    Hotbar,
    Inventory,
}

pub trait UiLayout {
    fn mode(&self) -> UiMode;

    fn calculate_positions(&self, screen_size: (u32, u32)) -> Vec<WidgetTransform>;

    fn handle_click(&mut self, pos: (u32, u32)) -> Option<UiAction>;

    /// Nombre de slots que ce layout gère (pour la hotbar/inventory)
    fn slot_count(&self) -> usize {
        1
    }

    /// Espacement entre slots (en pixels)
    fn spacing(&self) -> u32 {
        0
    }
}
