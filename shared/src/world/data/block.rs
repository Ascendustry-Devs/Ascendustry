use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub enum BlockType {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
}

impl BlockType {
    pub fn from_id(id: u32) -> BlockType {
        match id {
            0 => BlockType::Air,
            1 => BlockType::Grass,
            2 => BlockType::Dirt,
            3 => BlockType::Stone,
            _ => BlockType::Dirt,
        }
    }

    pub fn texture_index(&self) -> u32 {
        match self {
            BlockType::Air => 0,
            BlockType::Grass => 0,
            BlockType::Dirt => 1,
            BlockType::Stone => 2,
        }
    }

    pub const fn to_u32(&self) -> u32 {
        *self as u32
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockInstance {
    pub id: u32,
}

impl BlockInstance {
    pub fn new(id: u32) -> BlockInstance {
        return BlockInstance { id: id };
    }

    pub fn air() -> BlockInstance {
        return BlockInstance { id: 0 };
    }

    pub fn is_air(&self) -> bool {
        return self.id == BlockInstance::air().id;
    }

    pub fn is_solid(&self) -> bool {
        return self.id != BlockInstance::air().id;
    }

    pub fn block_type(&self) -> BlockType {
        BlockType::from_id(self.id)
    }

    pub fn texture_index(&self) -> u32 {
        self.block_type().texture_index()
    }
}

pub struct BlockData {
    pub id: Option<u32>,
    pub id_str: String, // e.g.: "base:dirt"

    // properties: solid, hardness, resistance, ...
}

impl BlockData {
    pub fn new(id: &str) -> Self {
        Self {
            id: None,
            id_str: id.to_owned(),
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id.expect(&format!("BlockData with id_str \"{}\" was not registered.", self.id_str))
    }

    pub fn get_id_str(&self) -> &str {
        &self.id_str
    }
}

pub struct BlockManager {
    blocks: Vec<BlockData>,
    mapped_blocks: HashMap<String, u32>,
}

impl BlockManager {
    pub fn new() -> Self {
        let blocks = Vec::with_capacity(256);
        let mapped_blocks = HashMap::with_capacity(256);
        Self { blocks, mapped_blocks }
    }

    pub fn block_count(&self) -> usize {
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
}
