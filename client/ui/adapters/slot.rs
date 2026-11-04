use std::marker::PhantomData;

use game::inventory::ItemStack;

use crate::ui::{adapters::UiAdapter, context::UiContext, views::slot::SlotView};

pub struct SlotAdapter<'a> {
    phantom: PhantomData<&'a ()>,
}

impl<'a> UiAdapter for SlotAdapter<'a> {
    type Input = &'a ItemStack;
    type Output = SlotView;

    fn adapt(input: Self::Input, ctx: &UiContext) -> Self::Output {
        let item_id = input.item().get_item().get_id();
        let texture = ctx.assets.get_item_texture(&item_id);
        let quantity = input.quantity();
        SlotView { texture, quantity }
    }
}
