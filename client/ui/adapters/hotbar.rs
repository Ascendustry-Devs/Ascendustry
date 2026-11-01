use game::inventory::Inventory;

use crate::ui::{
    adapters::UiAdapter,
    context::UiContext,
    views::hotbar::{HotbarView, SlotView},
};

pub struct HotbarAdapter;

impl UiAdapter for HotbarAdapter {
    type Input = Inventory;
    type Output = HotbarView;

    fn adapt(input: Self::Input, ctx: &UiContext) -> Self::Output {
        let slots = (0..8)
            .map(|i| {
                input.get_slot(i).map(|stack| {
                    let item_id = stack.item().get_item().get_id();
                    let icon = ctx.assets.get_item_texture(item_id);
                    let quantity = stack.quantity();
                    SlotView {
                        icon: icon,
                        quantity: quantity,
                    }
                })
            })
            .collect();

        HotbarView { slots: slots }
    }
}
