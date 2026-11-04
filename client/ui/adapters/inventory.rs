use std::marker::PhantomData;

use game::inventory::Inventory;

use crate::ui::{
    adapters::{slot::SlotAdapter, UiAdapter},
    views::inventory::InventoryView,
};

pub struct InventoryAdapter<'a> {
    phantom: PhantomData<&'a ()>,
}

impl<'a> UiAdapter for InventoryAdapter<'a> {
    type Input = (&'a Inventory, usize);
    type Output = InventoryView;

    fn adapt(input: Self::Input, ctx: &crate::ui::context::UiContext) -> Self::Output {
        let (inventory, selected) = input;
        let slots = (0..inventory.slot_count())
            .map(|i| inventory.get_slot(i).map(|stack| SlotAdapter::adapt(stack, ctx)))
            .collect();

        InventoryView { slots, selected }
    }
}
