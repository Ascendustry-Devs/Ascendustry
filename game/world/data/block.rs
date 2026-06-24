use std::collections::HashMap;
use std::path::Path;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::assets::block_loader::{block_id_str_to_item, load_block_definitions};
use crate::inventory::Item;

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
        Self {
            blocks,
            mapped_blocks,
            texture_lookup,
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

    pub fn block_to_item(&self, block_id: u32) -> Option<Item> {
        let block = self.get_block_by_id(block_id)?;
        if let Some(ref drop_str) = block.drop {
            return block_id_str_to_item(drop_str);
        }
        if block.has_item {
            return block_id_str_to_item(&block.id_str);
        }
        None
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
    }
}
