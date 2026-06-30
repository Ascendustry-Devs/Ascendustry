use std::collections::HashMap;

use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemInstance {
    pub id: u32,
}

impl ItemInstance {
    pub const fn new(id: u32) -> Self {
        Self { id }
    }

    pub const fn get_id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct ItemDefinition {
    pub id: Option<u32>,
    pub id_str: String,
    pub name: String,
    pub max_stack: u32,
    pub item_type: String,
    pub texture_path: Option<String>,
    pub texture_index: Option<u32>,
}

impl ItemDefinition {
    pub fn get_id(&self) -> u32 {
        self.id
            .unwrap_or_else(|| panic!("ItemDefinition with id_str \"{}\" was not registered.", self.id_str))
    }
}

pub struct ItemManager {
    items: Vec<ItemDefinition>,
    mapped_items: FxHashMap<String, u32>,
}

impl Default for ItemManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemManager {
    pub fn new() -> Self {
        Self {
            items: Vec::with_capacity(64),
            mapped_items: HashMap::with_hasher(FxBuildHasher),
        }
    }

    /// Enregistre un nouvel item dans le gestionnaire.
    pub fn register(&mut self, mut item: ItemDefinition) {
        if self.mapped_items.contains_key(&item.id_str) {
            panic!(
                "ItemManager: trying to insert a new item but its id_str is already registered: \"{}\"",
                item.id_str
            );
        }

        let id = self.items.len() as u32;
        item.id = Some(id);
        self.mapped_items.insert(item.id_str.clone(), id);
        self.items.push(item);
    }

    /// Retourne l'item correspondant à l'identifiant donné, s'il existe.
    pub fn get_by_id(&self, id: u32) -> Option<&ItemDefinition> {
        self.items.get(id as usize)
    }

    /// Retourne l'item correspondant à l'identifiant donné, s'il existe.
    pub fn get_by_string(&self, id_str: &str) -> Option<&ItemDefinition> {
        self.mapped_items.get(id_str).and_then(|id| self.get_by_id(*id))
    }

    /// Retourne l'item correspondant à l'identifiant donné, s'il existe.
    pub fn get_id_by_string(&self, id_str: &str) -> Option<u32> {
        self.mapped_items.get(id_str).copied()
    }

    /// Retourne le nombre d'items actuellement gérés par le gestionnaire.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Supprime tous les items du gestionnaire, effaçant ainsi les données en mémoire.
    pub fn dispose(&mut self) {
        self.items.clear();
        self.mapped_items.clear();
    }
}
