use std::collections::HashMap;
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize};

use crate::assets::block_loader::load_block_definitions;
use crate::inventory::item_manager::{ItemId, ItemInstance, ItemManager};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable, serde::Serialize, serde::Deserialize)]
pub struct BlockInstance {
    pub id: BlockId,
}

impl BlockInstance {
    pub const fn new(id: BlockId) -> Self {
        Self { id }
    }

    pub const fn air() -> Self {
        Self { id: BlockId(0) }
    }

    pub const fn is_air(&self) -> bool {
        self.id.0 == Self::air().id.0
    }

    pub const fn is_solid(&self) -> bool {
        self.id.0 != Self::air().id.0
    }

    pub const fn get_block_id(&self) -> BlockId {
        self.id
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u32);

impl From<BlockId> for u32 {
    fn from(value: BlockId) -> Self {
        value.0
    }
}

impl From<BlockId> for u16 {
    fn from(value: BlockId) -> Self {
        value.0 as u16
    }
}

impl From<BlockId> for usize {
    fn from(value: BlockId) -> Self {
        value.0 as usize
    }
}

#[derive(Debug, Clone)]
pub struct BlockData {
    pub id: Option<BlockId>,
    pub id_str: String,
    pub name: String,
    pub solid: bool,
    pub hardness: f32,
    pub texture_path: Option<String>,
    pub render_mode: String,
    pub drop: Option<String>,
    pub has_item: bool,
}

impl BlockData {
    pub fn get_id(&self) -> BlockId {
        self.id
            .unwrap_or_else(|| panic!("BlockData with id_str \"{}\" was not registered.", self.id_str))
    }

    pub fn get_id_str(&self) -> &str {
        &self.id_str
    }
}

#[derive(Debug)]
pub struct BlockManager {
    blocks: Vec<BlockData>,
    mapped_blocks: FxHashMap<String, BlockId>,
    item_to_block: FxHashMap<ItemId, BlockId>,
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
        let item_to_block = HashMap::with_hasher(FxBuildHasher);
        Self {
            blocks,
            mapped_blocks,
            item_to_block,
        }
    }

    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), String> {
        let defs = load_block_definitions(dir)?;
        for def in defs {
            self.register(def);
        }
        Ok(())
    }

    pub const fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn get_block_by_id(&self, id: &BlockId) -> Option<&BlockData> {
        self.blocks.get(id.0 as usize)
    }

    pub fn get_block_by_string(&self, id_str: String) -> Option<&BlockData> {
        if let Some(id) = self.mapped_blocks.get(&id_str) {
            return self.get_block_by_id(id);
        }
        None
    }

    pub fn register(&mut self, mut block: BlockData) -> BlockId {
        if self.mapped_blocks.contains_key(&block.id_str) {
            panic!(
                "BlockManager: trying to insert a new block but its id_str is already registered: \"{}\"",
                block.id_str
            );
        }

        let id = BlockId(self.block_count() as u32);
        block.id = Some(id);
        self.mapped_blocks.insert(block.id_str.clone(), id);
        self.blocks.push(block);
        id
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

    pub fn block_to_item(&self, block_id: &BlockId, item_manager: &ItemManager) -> Option<ItemInstance> {
        let block = self.get_block_by_id(block_id)?;
        let item_str = block
            .drop
            .as_ref()
            .or_else(|| if block.has_item { Some(&block.id_str) } else { None })?;
        item_manager.get_id_by_string(item_str).map(ItemInstance::new)
    }

    pub fn item_to_block_id(&self, item_id: &ItemId) -> Option<BlockId> {
        println!("{:?}", self.item_to_block);
        self.item_to_block.get(item_id).copied()
    }

    pub fn dispose(&mut self) {
        self.blocks.clear();
        self.mapped_blocks.clear();
        self.item_to_block.clear();
    }
}
