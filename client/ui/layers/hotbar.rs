use engine::render::ui::widgets::{DrawCommand, WidgetTransform};

use crate::ui::{
    layers::UiLayer,
    layouts::hotbar::{HotbarLayout, SlotLayout},
    views::hotbar::HotbarView,
};

#[derive(Debug)]
pub struct HotbarLayer {
    pub cols: u32,
    pub spacing: u32,
    pub view: HotbarView,
}

impl HotbarLayer {
    pub fn new(view: HotbarView) -> Self {
        Self {
            cols: 1,
            spacing: 0,
            view,
        }
    }
}

impl UiLayer for HotbarLayer {
    type Layout = HotbarLayout;

    fn layout(&self, available_space: &(u32, u32)) -> Self::Layout {
        let (mut w, mut h) = available_space;
        w /= 2;
        h /= 8;

        let hotbar_x = available_space.0 - 3 * (w >> 1);
        let hotbar_y = available_space.1 - h - self.spacing;
        let hotbar_width = w;
        let hotbar_height = h;

        let transform = WidgetTransform::new(hotbar_x, hotbar_y, hotbar_width, hotbar_height);

        w -= 2 * self.spacing;
        h -= 2 * self.spacing;

        // Hotbar en bas de l'écran, centré
        let slot_count = self.cols.max(8);
        let slot_size = (w.max(h).saturating_sub((slot_count - 1) * self.spacing)) / slot_count;
        let slot_start_x = hotbar_x + self.spacing;
        let slot_start_y = hotbar_y + self.spacing;

        let mut slots = Vec::new();

        for i in 0..slot_count {
            let x = slot_start_x + i * (slot_size + self.spacing);
            let slot = SlotLayout {
                transform: WidgetTransform::new(x, slot_start_y, slot_size, slot_size),
                icon: WidgetTransform::new(x + 2, slot_start_y + 2, slot_size - 4, slot_size - 4),
            };
            slots.push(slot);
        }

        HotbarLayout { transform, slots: slots }
    }

    fn draw(&self, layout: &Self::Layout) -> Vec<DrawCommand> {
        let HotbarView {
            slots: ref slots_views,
            selected,
        } = self.view;
        let HotbarLayout {
            transform,
            slots: slots_layouts,
        } = layout;

        let mut commands = Vec::<DrawCommand>::new();

        commands.push(DrawCommand::Panel {
            transform: *transform,
            color: 0x800000FF,
        });

        for (i, (slot_layout, slot_view)) in slots_layouts.iter().zip(slots_views).enumerate() {
            commands.push(DrawCommand::Panel {
                transform: slot_layout.transform,
                color: if i == selected { 0xFF0000FF } else { 0xAA333333 },
            });

            if let Some(view) = slot_view {
                let transform = slot_layout.icon;
                let texture = view.icon;
                commands.push(DrawCommand::TexturedPanel { transform, texture });
            }
        }

        commands
    }
}
