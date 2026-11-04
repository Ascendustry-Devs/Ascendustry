use std::marker::PhantomData;

use game::inventory::Inventory;

use crate::ui::{
    adapters::{slot::SlotAdapter, UiAdapter},
    context::UiContext,
    views::hotbar::HotbarView,
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
            .map(|i| inventory.get_slot(i).map(|stack| SlotAdapter::adapt(stack, ctx)))
            .collect();

        HotbarView { slots, selected }
    }
}
