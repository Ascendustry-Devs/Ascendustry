mod tests;

use std::fmt::Display;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

pub const DEFAULT_INVENTORY_SIZE: usize = 16;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum ItemType {
    Weapon,
    Placeable,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum Item {
    Dirt,
    Sword,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotData {
    pub index: usize,
    pub item: ItemStack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRules {
    pub max_quantity_per_stack: FxHashMap<ItemType, u32>,
    pub item_type: FxHashMap<Item, ItemType>,
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

    pub fn add_rule(&mut self, item: Item, item_type: ItemType, max_quantity_per_stack: u32) {
        self.item_type.insert(item, item_type);
        self.max_quantity_per_stack.insert(item_type, max_quantity_per_stack);
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ItemData {
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ItemStack {
    item: ItemData,
    quantity: u32,
}
impl ItemStack {
    /// Retourne true si l'item peut être empilé avec l'autre item, false sinon.
    pub fn new(item: ItemData, quantity: u32) -> Self {
        Self { item, quantity }
    }
    /// Retourne true si l'item peut être empilé avec l'autre item, false sinon.
    pub fn can_stack_with(&self, other: &ItemStack) -> bool {
        self.item == other.item
    }

    pub fn stack_with(&mut self, other: &mut ItemStack) {
        self.quantity += other.quantity;
        other.quantity = 0;
    }

    /// Ajoute une quantité à l'item existant dans ce slot.
    pub fn add(&mut self, quantity: u32) {
        self.quantity += quantity;
    }

    /// Retire une quantité de l'item existant dans ce slot.
    pub fn remove(&mut self, quantity: u32) {
        self.quantity = self.quantity.saturating_sub(quantity);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    slot_data: Vec<ItemStack>,
    max_slot_number: usize,
}
impl Inventory {
    /// Crée un inventaire vide avec la capacité maximale spécifiée.
    pub fn default(max_slot_number: usize) -> Self {
        Self {
            slot_data: Vec::with_capacity(max_slot_number),
            max_slot_number,
        }
    }

    /// Ajoute un item à l'inventaire en respectant les limites de stack.
    /// Retourne le SlotData modifié/créé, ou None si inventaire plein.
    pub fn add_item(&mut self, item: ItemData, quantity: u32, rules: &ItemRules) -> Option<SlotData> {
        let max = rules
            .max_quantity_per_stack
            .get(&item.get_item_type(rules))
            .copied()
            .unwrap_or(u32::MAX);

        for (index, slot) in self.slot_data.iter_mut().enumerate() {
            if slot.can_stack_with(&ItemStack::new(item.clone(), quantity)) && slot.quantity < max {
                let to_add = quantity.min(max - slot.quantity);
                slot.add(to_add);
                return Some(SlotData {
                    index,
                    item: slot.clone(),
                });
            }
        }

        if self.is_remaining_free_slot() {
            let to_add = quantity.min(max);
            let stack = ItemStack::new(item, to_add);
            self.slot_data.push(stack.clone());
            return Some(SlotData {
                index: self.slot_data.len() - 1,
                item: stack,
            });
        }

        None
    }

    /// Supprime un item de l'inventaire à partir d'un slot spécifié.
    pub fn remove_item(&mut self, _item: ItemData, quantity: u32, selected_slot: usize) {
        if let Some(slot) = self.slot_data.get_mut(selected_slot) {
            slot.remove(quantity);
            if slot.quantity == 0 {
                self.slot_data.remove(selected_slot);
            }
        }
    }

    /// Retourne le slot à l'index spécifié, s'il existe.
    pub fn get_slot(&self, slot: usize) -> Option<&ItemStack> {
        if self.is_slot_correct(slot) {
            Some(&self.slot_data[slot])
        } else {
            None
        }
    }

    /// Échange les slots spécifiés.
    pub fn swap_slots(&mut self, slot1: usize, slot2: usize) {
        if self.is_slot_correct(slot1) && self.is_slot_correct(slot2) {
            self.slot_data.swap(slot1, slot2);
        }
    }

    pub fn update_slots(&mut self, modified_slots: Vec<SlotData>) {
        for slot in modified_slots {
            if self.is_slot_correct(slot.index) {
                self.slot_data[slot.index] = slot.item;
            }
        }
    }

    /// Retourne le nombre de slots libres dans l'inventaire.
    pub fn free_slots_count(&self) -> usize {
        self.max_slot_number.saturating_sub(self.slot_data.len())
    }

    /// Retourne le nombre de slots utilisés dans l'inventaire.
    pub fn slot_count(&self) -> usize {
        self.slot_data.len()
    }

    /// Efface tous les items de l'inventaire.
    pub fn clear(&mut self) {
        self.slot_data.clear();
    }

    /// Retourne true si l'inventaire est vide, false sinon.
    pub fn is_empty(&self) -> bool {
        self.slot_data.is_empty()
    }

    /// Retourne true si l'inventaire est plein, false sinon.
    pub fn is_full(&self) -> bool {
        self.slot_data.len() == self.max_slot_number
    }

    /// Retourne tous les items de l'inventaire.
    pub fn get_all_items(&self) -> Vec<(ItemData, u32)> {
        self.slot_data.iter().map(|s| (s.item.clone(), s.quantity)).collect()
    }

    /// Retourne true si le slot est correct, false sinon.
    pub fn is_slot_correct(&self, slot: usize) -> bool {
        slot < self.slot_data.len() && slot < self.max_slot_number
    }

    /// Retourne true s'il reste un slot libre, false sinon.
    pub fn is_remaining_free_slot(&self) -> bool {
        self.slot_data.len() < self.max_slot_number
    }

    /// Retourne la quantité d'un slot donné.
    pub fn get_slot_quantity(&self, slot: usize) -> u32 {
        if self.is_slot_correct(slot) {
            self.slot_data[slot].quantity
        } else {
            0
        }
    }

    /// Retourne la quantité d'un item donné.
    pub fn get_item_quantity(&self, item: &ItemData) -> u32 {
        self.slot_data.iter().filter(|s| s.item == *item).map(|s| s.quantity).sum()
    }

    /// Libère tous les slots vides de l'inventaire.
    pub fn retain(&mut self) {
        self.slot_data.retain(|slot| slot.quantity > 0);
    }
}

impl Display for Inventory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Inventory: [")?;
        for (i, slot) in self.slot_data.iter().enumerate() {
            write!(f, "{}: {}", i, slot.quantity)?;
            if i < self.slot_data.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}
