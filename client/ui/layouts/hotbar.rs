use engine::render::ui::widgets::{DrawCommand, WidgetTransform};

use crate::ui::{
    action::UiAction,
    context::UiContext,
    layer::UiLayer,
    layouts::{UiLayout, UiMode},
};

pub struct Hotbar {
    pub cols: u32,
    pub spacing: u32,
}

impl UiLayout for Hotbar {
    fn mode(&self) -> UiMode {
        UiMode::Hotbar
    }

    fn calculate_positions(&self, screen_size: (u32, u32)) -> Vec<WidgetTransform> {
        let (w, h) = screen_size;
        if w == 0 || h == 0 {
            return vec![];
        }

        // Hotbar en bas de l'écran, centré
        let slot_count = self.cols.max(8);
        let slot_size = (w.saturating_sub((slot_count - 1) * self.spacing)) / slot_count;
        let start_x = (w.saturating_sub(slot_size * slot_count + self.spacing * (slot_count - 1))) / 2;
        let start_y = h.saturating_sub(slot_size + 8);

        let mut transforms = Vec::with_capacity(slot_count as usize);

        for i in 0..slot_count {
            let x = start_x + i as u32 * (slot_size + self.spacing);
            transforms.push(WidgetTransform::new(x, start_y, slot_size, slot_size));
        }
        transforms
    }

    fn handle_click(&mut self, pos: (u32, u32)) -> Option<UiAction> {
        None
    }

    fn slot_count(&self) -> usize {
        self.cols as usize
    }

    fn spacing(&self) -> u32 {
        self.spacing
    }
}

#[derive(Debug)]
pub struct SlotView {
    icon: u32,
    quantity: u32,
}

#[derive(Debug)]
pub struct HotbarView {
    slots: Vec<Option<SlotView>>,
}

#[derive(Debug)]
pub struct HotbarLayer {
    pub cols: u32,
    pub spacing: u32,

    view: HotbarView,

    cached_layout: Vec<WidgetTransform>,
    cached_draw_commands: Vec<DrawCommand>,
}

impl HotbarLayer {
    pub fn new(view: HotbarView) -> Self {
        Self {
            cols: 1,
            spacing: 0,
            view: view,
            cached_layout: vec![],
            cached_draw_commands: vec![],
        }
    }
}

impl UiLayer for HotbarLayer {
    fn layout(&mut self, available_space: &(u32, u32)) -> &Vec<WidgetTransform> {
        let (w, h) = available_space;

        // Hotbar en bas de l'écran, centré
        let slot_count = self.cols.max(8);
        let slot_size = (w.saturating_sub((slot_count - 1) * self.spacing)) / slot_count;
        let start_x = (w.saturating_sub(slot_size * slot_count + self.spacing * (slot_count - 1))) / 2;
        let start_y = h.saturating_sub(slot_size + 8);

        for i in 0..slot_count {
            let x = start_x + i as u32 * (slot_size + self.spacing);
            self.cached_layout
                .push(WidgetTransform::new(x, start_y, slot_size, slot_size));
        }

        &self.cached_layout
    }

    fn update(&mut self, available_space: &(u32, u32), ctx: &UiContext) -> &Vec<WidgetTransform> {
        for i in 0..(self.cols as usize) {
            let current_slot_view = &mut self.view.slots[i];
            let actual_data = ctx.players.browse_slot(i);

            if current_slot_view.is_none() && actual_data.is_none() {
                continue;
            }
            if actual_data.is_none() {
                *current_slot_view = None;
                continue;
            }

            let item_stack = actual_data.unwrap();

            let icon = 0; // TODO

            let slot_view = SlotView {
                icon: icon,
                quantity: item_stack.quantity(),
            };

            *current_slot_view = Some(slot_view);
        }

        self.layout(available_space);

        &self.cached_layout
    }

    fn draw(&mut self, layout: &Vec<WidgetTransform>, ctx: &UiContext) -> &Vec<DrawCommand> {
        todo!()
    }
}
