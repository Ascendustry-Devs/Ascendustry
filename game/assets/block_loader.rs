use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::inventory::Item;
use crate::world::data::block::BlockData;

#[derive(Deserialize)]
struct BlockDefinitionRaw {
    id: String,
    name: String,
    #[serde(default = "default_solid")]
    solid: bool,
    #[serde(default = "default_hardness")]
    hardness: f32,
    #[serde(default)]
    texture: Option<String>,
    #[serde(default = "default_render_mode_str")]
    render_mode: String,
    #[serde(default)]
    drop: Option<String>,
    #[serde(default = "default_true")]
    has_item: bool,
}

fn default_solid() -> bool {
    true
}
fn default_hardness() -> f32 {
    0.5
}
fn default_render_mode_str() -> String {
    "opaque".to_string()
}
fn default_true() -> bool {
    true
}

impl BlockDefinitionRaw {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Block id cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Block name cannot be empty".to_string());
        }
        if self.hardness < 0.0 {
            return Err(format!("Block '{}' has negative hardness", self.id));
        }

        Ok(())
    }
}

pub fn load_block_definitions<P: AsRef<Path>>(dir: P) -> Result<Vec<BlockData>, String> {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return Err(format!("Block definitions directory not found: {:?}", dir));
    }

    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read block directory {:?}: {}", dir, e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map(|ext| ext == "json").unwrap_or(false))
        .collect();

    // On trie par ordre alphabétique pour avoir des id fixes
    entries.sort_by_key(|a| a.file_name());

    let mut blocks = Vec::with_capacity(entries.len());

    for entry in &entries {
        let content = fs::read_to_string(entry.path()).map_err(|e| format!("Failed to read {:?}: {}", entry.path(), e))?;

        let raw: BlockDefinitionRaw =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse {:?}: {}", entry.path(), e))?;

        raw.validate().map_err(|e| format!("{:?}: {}", entry.path(), e))?;

        let block = BlockData {
            id: None,
            id_str: raw.id,
            name: raw.name,
            solid: raw.solid,
            hardness: raw.hardness,
            texture_path: raw.texture,
            texture_index: None,
            render_mode: raw.render_mode,
            drop: raw.drop,
            has_item: raw.has_item,
        };
        blocks.push(block);
    }

    if blocks.is_empty() {
        return Err("No block definitions found".to_string());
    }

    Ok(blocks)
}

pub fn block_id_str_to_item(id_str: &str) -> Option<Item> {
    match id_str {
        "dirt" => Some(Item::Dirt),
        "grass" => Some(Item::Grass),
        "stone" => Some(Item::Stone),
        _ => None,
    }
}
