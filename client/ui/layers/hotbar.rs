use engine::render::ui::widgets::{DrawCommand, WidgetTransform};

use crate::ui::{
    adapters::{hotbar::HotbarAdapter, UiAdapter},
    context::UiContext,
    layers::UiLayer,
    layouts::{hotbar::HotbarLayout, slot::SlotLayout},
    views::hotbar::HotbarView,
};

#[derive(Debug, Clone)]
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

    fn update(&mut self, ctx: &UiContext) {
        let input = (ctx.players.browse_local_inventory(), ctx.players.browse_selected_slot().0);
        self.view = HotbarAdapter::adapt(input, ctx);
    }

    fn layout(&self, available_space: &(u32, u32)) -> Self::Layout {
        let (mut w, mut h) = available_space;
        w = ((w as f32) * 3.0 / 4.0) as u32;
        h /= 8;

        w = w.saturating_sub(2 * self.spacing);
        h = h.saturating_sub(2 * self.spacing);

        // Hotbar en bas de l'écran, centré
        let slot_count = self.cols;
        let mut slot_size = (w.max(h).saturating_sub((slot_count - 1) * self.spacing)) / slot_count;
        slot_size = slot_size.clamp(1, available_space.1 / 3);

        let hotbar_x = (available_space.0 - w) >> 1;
        let hotbar_y = available_space.1 - (slot_size + 2 * self.spacing);

        let hotbar_width = w.saturating_sub(self.spacing);
        let hotbar_height = slot_size + 2 * self.spacing;

        let transform = WidgetTransform::new(hotbar_x, hotbar_y, hotbar_width, hotbar_height);

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

        HotbarLayout { transform, slots }
    }

    fn draw(&self, layout: &Self::Layout) -> Vec<DrawCommand> {
        const BACKGROUND_COLOR: u32 = 0x00000080;
        const SLOT_COLOR: u32 = 0x333333AA;
        const SELECTION_COLOR: u32 = 0x008080FF;

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
            color: BACKGROUND_COLOR,
        });

        for (i, (slot_layout, slot_view)) in slots_layouts.iter().zip(slots_views).enumerate() {
            commands.push(DrawCommand::Panel {
                transform: slot_layout.transform,
                color: if i == selected { SELECTION_COLOR } else { SLOT_COLOR },
            });

            if let Some(view) = slot_view {
                let transform = slot_layout.icon;
                let texture = view.texture;
                commands.push(DrawCommand::TexturedPanel { transform, texture });
            }
        }

        commands
    }
}
