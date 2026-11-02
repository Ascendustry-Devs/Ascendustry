use std::marker::PhantomData;

use game::inventory::Inventory;

use crate::ui::{
    adapters::UiAdapter,
    context::UiContext,
    views::hotbar::{HotbarView, SlotView},
};

pub struct HotbarAdapter<'a> {
    phantom: PhantomData<&'a ()>,
}

impl<'a> UiAdapter for HotbarAdapter<'a> {
    type Input = (&'a Inventory, usize);
    type Output = HotbarView;

    fn adapt(input: Self::Input, ctx: &UiContext) -> Self::Output {
        let (inventory, selected) = input;
        let slots = (0..8)
            .map(|i| {
                inventory.get_slot(i).map(|stack| {
                    let item_id = stack.item().get_item().get_id();
                    let texture = ctx.assets.get_item_texture(&item_id);
                    let quantity = stack.quantity();
                    SlotView { texture, quantity }
                })
            })
            .collect();

        HotbarView { slots, selected }
    }
}
