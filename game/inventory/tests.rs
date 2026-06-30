#[cfg(test)]
mod tests {
    use crate::inventory::item_manager::ItemInstance;
    use crate::inventory::{Inventory, ItemData, ItemRules, ItemStack, ItemType};

    const DIRT_ID: u32 = 0;
    const SWORD_ID: u32 = 1;

    fn dirt_item() -> ItemData {
        ItemData::new(ItemInstance::new(DIRT_ID), None)
    }

    fn sword_item() -> ItemData {
        ItemData::new(ItemInstance::new(SWORD_ID), None)
    }

    fn make_rules() -> ItemRules {
        let mut rules = ItemRules::default();
        rules.add_rule(DIRT_ID, ItemType::Placeable, 96);
        rules.add_rule(SWORD_ID, ItemType::Weapon, 1);
        rules
    }

    #[test]
    fn test_item_data_new() {
        let data = dirt_item();
        assert_eq!(data.get_item(), ItemInstance::new(DIRT_ID));
        assert!(data.custom_name.is_none());

        let named = ItemData::new(ItemInstance::new(SWORD_ID), Some("Excalibur".into()));
        assert_eq!(named.get_item(), ItemInstance::new(SWORD_ID));
        assert_eq!(named.custom_name, Some("Excalibur".into()));
    }

    #[test]
    fn test_item_data_get_item_type() {
        let rules = make_rules();
        let dirt = dirt_item();
        let sword = sword_item();

        assert_eq!(dirt.get_item_type(&rules), ItemType::Placeable);
        assert_eq!(sword.get_item_type(&rules), ItemType::Weapon);
    }

    #[test]
    fn test_item_data_modify_custom_name() {
        let mut data = dirt_item();
        data.modify_custom_name(Some("Magic Dirt".into()));
        assert_eq!(data.custom_name, Some("Magic Dirt".into()));

        data.modify_custom_name(None);
        assert!(data.custom_name.is_none());
    }

    #[test]
    fn test_item_stack_new() {
        let stack = ItemStack::new(dirt_item(), 10);
        assert_eq!(stack.item().get_item(), ItemInstance::new(DIRT_ID));
        assert_eq!(stack.quantity(), 10);
    }

    #[test]
    fn test_item_stack_can_stack_with() {
        let dirt = ItemStack::new(dirt_item(), 5);
        let same = ItemStack::new(dirt_item(), 3);
        let sword = ItemStack::new(sword_item(), 1);

        assert!(dirt.can_stack_with(&same));
        assert!(!dirt.can_stack_with(&sword));
    }

    #[test]
    fn test_item_stack_stack_with() {
        let mut a = ItemStack::new(dirt_item(), 30);
        let mut b = ItemStack::new(dirt_item(), 20);

        a.stack_with(&mut b);

        assert_eq!(a.quantity(), 50);
        assert_eq!(b.quantity(), 0);
    }

    #[test]
    fn test_item_stack_add() {
        let mut stack = ItemStack::new(dirt_item(), 10);
        stack.add(5);
        assert_eq!(stack.quantity(), 15);
    }

    #[test]
    fn test_item_stack_remove() {
        let mut stack = ItemStack::new(dirt_item(), 10);
        stack.remove(3);
        assert_eq!(stack.quantity(), 7);
    }

    #[test]
    fn test_item_stack_remove_saturating() {
        let mut stack = ItemStack::new(dirt_item(), 5);
        stack.remove(10);
        assert_eq!(stack.quantity(), 0);
    }

    #[test]
    fn test_inventory_default() {
        let inv = Inventory::default(10);
        assert!(inv.is_empty());
        assert_eq!(inv.slot_count(), 0);
        assert_eq!(inv.free_slots_count(), 10);
    }

    #[test]
    fn test_inventory_add_item_new_slot() {
        let mut inv = Inventory::default(5);
        let item = dirt_item();
        let rules = make_rules();

        let returned = inv.add_item(item, 10, &rules);

        assert!(returned.is_some());
        assert_eq!(inv.slot_count(), 1);
        assert_eq!(inv.get_slot_quantity(0), 10);
    }

    #[test]
    fn test_inventory_add_item_stacks_existing() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(dirt_item(), 20, &rules);

        assert_eq!(inv.slot_count(), 1);
        assert_eq!(inv.get_slot_quantity(0), 30);
    }

    #[test]
    fn test_inventory_add_item_different_items_separate_slots() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(sword_item(), 1, &rules);

        assert_eq!(inv.slot_count(), 2);
        assert_eq!(inv.get_slot_quantity(0), 10);
        assert_eq!(inv.get_slot_quantity(1), 1);
    }

    #[test]
    fn test_inventory_remove_item() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.remove_item(dirt_item(), 4, 0);

        assert_eq!(inv.get_slot_quantity(0), 6);
        assert_eq!(inv.slot_count(), 1);
    }

    #[test]
    fn test_inventory_remove_item_removes_empty_slot() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 5, &rules);
        inv.remove_item(dirt_item(), 5, 0);

        assert!(inv.is_empty());
        assert_eq!(inv.slot_count(), 0);
    }

    #[test]
    fn test_inventory_remove_item_invalid_slot() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 5, &rules);
        inv.remove_item(dirt_item(), 1, 99);
        assert_eq!(inv.slot_count(), 1);
    }

    #[test]
    fn test_inventory_get_slot() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);

        let slot = inv.get_slot(0);
        assert!(slot.is_some());
        assert_eq!(slot.unwrap().quantity(), 10);
    }

    #[test]
    fn test_inventory_get_slot_invalid() {
        let inv = Inventory::default(5);
        assert!(inv.get_slot(0).is_none());
        assert!(inv.get_slot(99).is_none());
    }

    #[test]
    fn test_inventory_swap_slots() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(sword_item(), 1, &rules);

        inv.swap_slots(0, 1);

        assert_eq!(inv.get_slot_quantity(0), 1);
        assert_eq!(inv.get_slot_quantity(1), 10);
    }

    #[test]
    fn test_inventory_swap_slots_invalid() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);

        inv.swap_slots(0, 99);
        assert_eq!(inv.slot_count(), 1);
        assert_eq!(inv.get_slot_quantity(0), 10);
    }

    #[test]
    fn test_inventory_slot_count() {
        let mut inv = Inventory::default(10);
        let rules = make_rules();
        assert_eq!(inv.slot_count(), 0);
        inv.add_item(dirt_item(), 5, &rules);
        assert_eq!(inv.slot_count(), 1);
        inv.add_item(sword_item(), 1, &rules);
        assert_eq!(inv.slot_count(), 2);
    }

    #[test]
    fn test_inventory_clear() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(sword_item(), 1, &rules);

        inv.clear();
        assert!(inv.is_empty());
        assert_eq!(inv.slot_count(), 0);
    }

    #[test]
    fn test_inventory_is_empty() {
        let inv = Inventory::default(5);
        assert!(inv.is_empty());

        let mut inv2 = Inventory::default(5);
        let rules = make_rules();
        inv2.add_item(dirt_item(), 1, &rules);
        assert!(!inv2.is_empty());
    }

    #[test]
    fn test_inventory_is_full() {
        let mut inv = Inventory::default(2);
        let rules = make_rules();
        assert!(!inv.is_full());

        inv.add_item(dirt_item(), 1, &rules);
        assert!(!inv.is_full());

        inv.add_item(sword_item(), 1, &rules);
        assert!(inv.is_full());
    }

    #[test]
    fn test_inventory_get_all_items() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(sword_item(), 1, &rules);

        let items = inv.get_all_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0.get_item(), ItemInstance::new(DIRT_ID));
        assert_eq!(items[0].1, 10);
        assert_eq!(items[1].0.get_item(), ItemInstance::new(SWORD_ID));
        assert_eq!(items[1].1, 1);
    }

    #[test]
    fn test_inventory_is_slot_correct() {
        let mut inv = Inventory::default(3);
        let rules = make_rules();
        inv.add_item(dirt_item(), 5, &rules);
        inv.add_item(sword_item(), 1, &rules);

        assert!(inv.is_slot_correct(0));
        assert!(inv.is_slot_correct(1));
        assert!(!inv.is_slot_correct(2));
        assert!(!inv.is_slot_correct(3));
        assert!(!inv.is_slot_correct(99));
    }

    #[test]
    fn test_inventory_get_slot_quantity() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 42, &rules);

        assert_eq!(inv.get_slot_quantity(0), 42);
        assert_eq!(inv.get_slot_quantity(1), 0);
        assert_eq!(inv.get_slot_quantity(99), 0);
    }

    #[test]
    fn test_inventory_retain() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 5, &rules);
        inv.add_item(sword_item(), 1, &rules);

        inv.slot_data[0].remove(5);

        inv.retain();
        assert_eq!(inv.slot_count(), 1);
        assert_eq!(inv.get_slot_quantity(0), 1);
    }

    #[test]
    fn test_inventory_display_empty() {
        let inv = Inventory::default(5);
        assert_eq!(format!("{}", inv), "Inventory: []");
    }

    #[test]
    fn test_inventory_display_with_items() {
        let mut inv = Inventory::default(5);
        let rules = make_rules();
        inv.add_item(dirt_item(), 10, &rules);
        inv.add_item(sword_item(), 1, &rules);
        assert_eq!(format!("{}", inv), "Inventory: [0: 10, 1: 1]");
    }
}
