use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::inventory::item_manager::ItemDefinition;

#[derive(Deserialize)]
struct ItemDefinitionRaw {
    id: String,
    name: String,
    #[serde(default = "default_max_stack")]
    max_stack: u32,
    #[serde(default = "default_item_type")]
    item_type: String,
    #[serde(default)]
    texture: Option<String>,
}

fn default_max_stack() -> u32 {
    96
}

fn default_item_type() -> String {
    "Placeable".to_string()
}

impl ItemDefinitionRaw {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Item id cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Item name cannot be empty".to_string());
        }
        if self.max_stack == 0 {
            return Err(format!("Item '{}' has max_stack of 0", self.id));
        }
        Ok(())
    }
}

pub fn load_item_definitions<P: AsRef<Path>>(dir: P) -> Result<Vec<ItemDefinition>, String> {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return Err(format!("Item definitions directory not found: {:?}", dir));
    }

    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read item directory {:?}: {}", dir, e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map(|ext| ext == "json").unwrap_or(false))
        .collect();

    entries.sort_by_key(|a| a.file_name());

    let mut items = Vec::with_capacity(entries.len());

    for entry in &entries {
        let content = fs::read_to_string(entry.path()).map_err(|e| format!("Failed to read {:?}: {}", entry.path(), e))?;

        let raw: ItemDefinitionRaw =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse {:?}: {}", entry.path(), e))?;

        raw.validate().map_err(|e| format!("{:?}: {}", entry.path(), e))?;

        let item = ItemDefinition {
            id: None,
            id_str: raw.id,
            name: raw.name,
            max_stack: raw.max_stack,
            item_type: raw.item_type,
            texture_path: raw.texture,
            texture_index: None,
        };
        items.push(item);
    }

    if items.is_empty() {
        return Err("No item definitions found".to_string());
    }

    Ok(items)
}
