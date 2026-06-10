use rustc_hash::FxHashMap;

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
enum ItemType {
    Weapon,
    Placeable,
}

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
enum Item {
    Dirt,
    Sword,
}

struct ItemRules {
    max_quantity_per_stack: FxHashMap<ItemType, u32>,
    item_type: FxHashMap<Item, ItemType>,
}

impl ItemRules {
    pub fn default() -> Self {
        let mut stack_quantity = FxHashMap::default();
        stack_quantity.insert(ItemType::Placeable, 96);
        stack_quantity.insert(ItemType::Weapon, 1);

        let mut item_type = FxHashMap::default();
        item_type.insert(Item::Dirt, ItemType::Placeable);
        item_type.insert(Item::Sword, ItemType::Weapon);

        Self {
            max_quantity_per_stack: stack_quantity,
            item_type,
        }
    }
}

#[derive(Clone)]
struct ItemData {
    item: Item,
    custom_name: Option<String>,
}
impl ItemData {
    pub fn new(item: Item, custom_name: Option<String>) -> Self {
        Self { item, custom_name }
    }
    pub fn get_item_type(&self, item_rules: &ItemRules) -> ItemType {
        item_rules
            .item_type
            .get(&self.item)
            .copied()
            .expect("Item type not found, probably an item_rules is not initialized")
    }
    pub fn modify_custom_name(&mut self, custom_name: Option<String>) {
        self.custom_name = custom_name;
    }
}

#[derive(Clone)]
struct ItemStack {
    item: ItemData,
    quantity: u32,
}
impl ItemStack {
    pub fn new(item: ItemData, quantity: u32) -> Self {
        Self { item, quantity }
    }
    pub fn can_stack_with(&self, other: &ItemStack) -> bool {
        self.item.item == other.item.item
    }
    pub fn stack_with(&mut self, other: &mut ItemStack) {
        self.quantity += other.quantity;
        other.quantity = 0;
    }
    pub fn add(&mut self, quantity: u32) {
        self.quantity += quantity;
    }
    pub fn remove(&mut self, quantity: u32) {
        self.quantity = self.quantity.saturating_sub(quantity);
    }
}

struct Inventory {
    slot_data: Vec<ItemStack>,
    max_slot_number: usize,
}
impl Inventory {
    pub fn default(max_slot_number: usize) -> Self {
        Self {
            slot_data: Vec::with_capacity(max_slot_number),
            max_slot_number,
        }
    }
    pub fn add_item(&mut self, item: ItemData, quantity: u32) -> u32 {
        // Cherche slot existant avec même item
        for slot in &mut self.slot_data {
            if slot.can_stack_with(&ItemStack {
                item: item.clone(),
                quantity: 0,
            }) {
                slot.add(quantity);
                return quantity;
            }
        }

        // Pas de slot existant → nouveau slot
        self.slot_data.push(ItemStack::new(item, quantity));
        quantity
    }

    pub fn remove_item(&mut self, item: ItemData, quantity: u32, selected_slot: usize) {
        if let Some(slot) = self.slot_data.get_mut(selected_slot) {
            slot.remove(quantity);
            if slot.quantity == 0 {
                self.slot_data.remove(selected_slot);
            }
        }
    }

    pub fn get_slot(&self, slot: usize) -> Option<&ItemStack> {
        if self.is_slot_correct(slot) {
            Some(&self.slot_data[slot])
        } else {
            None
        }
    }

    pub fn swap_slots(&mut self, slot1: usize, slot2: usize) {
        if self.is_slot_correct(slot1) && self.is_slot_correct(slot2) {
            self.slot_data.swap(slot1, slot2);
        }
    }
    pub fn slot_count(&self) -> usize {
        self.slot_data.len()
    }
    pub fn clear(&mut self) {
        self.slot_data.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.slot_data.is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.slot_data.len() == self.max_slot_number
    }
    pub fn get_all_items(&self) -> Vec<(Item, u32)> {
        self.slot_data.iter().map(|s| (s.item.item, s.quantity)).collect()
    }

    pub fn is_slot_correct(&self, slot: usize) -> bool {
        slot < self.slot_data.len() && slot < self.max_slot_number
    }

    pub fn get_slot_quantity(&self, slot: usize) -> u32 {
        if self.is_slot_correct(slot) {
            self.slot_data[slot].quantity
        } else {
            0
        }
    }

    pub fn retain(&mut self) {
        self.slot_data.retain(|slot| slot.quantity > 0);
    }
}
