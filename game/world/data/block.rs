use std::collections::HashMap;
use std::path::Path;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::assets::block_loader::load_block_definitions;
use crate::inventory::item_manager::{ItemInstance, ItemManager};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockInstance {
    pub id: u32,
}

impl BlockInstance {
    pub const fn new(id: u32) -> Self {
        Self { id }
    }

    pub const fn air() -> Self {
        Self { id: 0 }
    }

    pub const fn is_air(&self) -> bool {
        self.id == Self::air().id
    }

    pub const fn is_solid(&self) -> bool {
        self.id != Self::air().id
    }

    pub const fn get_block_id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct BlockData {
    pub id: Option<u32>,
    pub id_str: String,
    pub name: String,
    pub solid: bool,
    pub hardness: f32,
    pub texture_path: Option<String>,
    pub texture_index: Option<u32>,
    pub render_mode: String,
    pub drop: Option<String>,
    pub has_item: bool,
}

impl BlockData {
    pub fn get_id(&self) -> u32 {
        self.id
            .unwrap_or_else(|| panic!("BlockData with id_str \"{}\" was not registered.", self.id_str))
    }

    pub fn get_id_str(&self) -> &str {
        &self.id_str
    }
}

pub struct BlockManager {
    blocks: Vec<BlockData>,
    mapped_blocks: FxHashMap<String, u32>,
    texture_lookup: Vec<u32>,
    item_to_block: FxHashMap<u32, u32>,
}

impl Default for BlockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockManager {
    pub fn new() -> Self {
        let blocks = Vec::with_capacity(256);
        let mapped_blocks = HashMap::with_capacity_and_hasher(256, FxBuildHasher);
        let texture_lookup = Vec::new();
        let item_to_block = HashMap::with_hasher(FxBuildHasher);
        Self {
            blocks,
            mapped_blocks,
            texture_lookup,
            item_to_block,
        }
    }

    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), String> {
        let defs = load_block_definitions(dir)?;
        for def in defs {
            self.register(def);
        }
        self.build_texture_lookup();
        Ok(())
    }

    pub const fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn get_block_by_id(&self, id: u32) -> Option<&BlockData> {
        self.blocks.get(id as usize)
    }

    pub fn get_block_by_string(&self, id_str: String) -> Option<&BlockData> {
        if let Some(id) = self.mapped_blocks.get(&id_str) {
            return self.get_block_by_id(*id);
        }
        None
    }

    pub fn register(&mut self, mut block: BlockData) {
        if self.mapped_blocks.contains_key(&block.id_str) {
            panic!(
                "BlockManager: trying to insert a new block but its id_str is already registered: \"{}\"",
                block.id_str
            );
        }

        let id = self.block_count() as u32;
        block.id = Some(id);
        self.mapped_blocks.insert(block.id_str.clone(), id);
        self.blocks.push(block);
    }

    pub fn resolve_item_mappings(&mut self, item_manager: &ItemManager) {
        for block in &self.blocks {
            let item_str = block
                .drop
                .as_ref()
                .or_else(|| if block.has_item { Some(&block.id_str) } else { None });

            if let Some(item_str) = item_str {
                if let Some(item_id) = item_manager.get_id_by_string(item_str) {
                    self.item_to_block.insert(item_id, block.get_id());
                }
            }
        }
    }

    pub fn block_to_item(&self, block_id: u32, item_manager: &ItemManager) -> Option<ItemInstance> {
        let block = self.get_block_by_id(block_id)?;
        let item_str = block
            .drop
            .as_ref()
            .or_else(|| if block.has_item { Some(&block.id_str) } else { None })?;
        item_manager.get_id_by_string(item_str).map(ItemInstance::new)
    }

    pub fn item_to_block_id(&self, item_id: u32) -> Option<u32> {
        self.item_to_block.get(&item_id).copied()
    }

    pub fn build_texture_lookup(&mut self) {
        self.texture_lookup = (0..self.block_count())
            .map(|id| self.get_block_by_id(id as u32).and_then(|b| b.texture_index).unwrap_or(0))
            .collect();
    }

    pub fn get_texture_lookup(&self) -> &[u32] {
        &self.texture_lookup
    }

    pub fn dispose(&mut self) {
        self.blocks.clear();
        self.mapped_blocks.clear();
        self.item_to_block.clear();
    }
}
