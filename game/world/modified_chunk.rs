use std::collections::HashMap;
use std::fmt;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::world::chunk_position::ChunkPosition;
use crate::world::data::block::BlockInstance;
use crate::world::local_position::LocalPosition;
use crate::world::world_position::WorldPosition;

#[derive(Debug)]
pub enum ModifiedWorldError {
    ValeurInvalide(ChunkPosition),
}

impl fmt::Display for ModifiedWorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValeurInvalide(position) => {
                write!(f, "Le chunk ({:?}) n'existe pas ou n'a pas été modifié", position)
            }
        }
    }
}

pub struct ModifiedChunk {
    blocks: Vec<(LocalPosition, BlockInstance)>,
    index: FxHashMap<LocalPosition, usize>,
}

impl Default for ModifiedChunk {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifiedChunk {
    pub const fn new() -> Self {
        Self {
            blocks: vec![],
            index: HashMap::with_hasher(FxBuildHasher),
        }
    }

    pub fn get_block_at(&self, coords: &LocalPosition) -> Option<&BlockInstance> {
        self.index
            .get(coords)
            .and_then(|&i| self.blocks.get(i))
            .map(|(_, block)| block)
    }

    pub fn set_block_at(&mut self, coords: &LocalPosition, block: BlockInstance) {
        if let Some(i) = self.index.get(coords) {
            self.blocks[*i].1 = block;
        } else {
            let i = self.blocks.len();
            self.blocks.push((*coords, block));
            self.index.insert(*coords, i);
        }
    }

    pub fn remove_block_at(&mut self, coords: &LocalPosition) -> Option<BlockInstance> {
        let i = self.index.remove(coords)?;
        let (_, removed_block) = self.blocks.swap_remove(i);

        if i < self.blocks.len() {
            let moved_coords = self.blocks[i].0;
            self.index.insert(moved_coords, i);
        }

        Some(removed_block)
    }
    pub fn blocks(&self) -> &[(LocalPosition, BlockInstance)] {
        &self.blocks
    }

    pub fn into_blocks(self) -> Vec<(LocalPosition, BlockInstance)> {
        self.blocks
    }

    pub fn from_blocks(blocks: Vec<(LocalPosition, BlockInstance)>) -> Self {
        let mut index = HashMap::with_capacity_and_hasher(blocks.len(), FxBuildHasher);
        for (i, (coords, _)) in blocks.iter().enumerate() {
            index.insert(*coords, i);
        }
        Self { blocks, index }
    }
}

pub struct ModifiedWorld {
    pub chunks: FxHashMap<ChunkPosition, ModifiedChunk>,
}

impl Default for ModifiedWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifiedWorld {
    pub const fn new() -> Self {
        Self {
            chunks: HashMap::with_hasher(FxBuildHasher),
        }
    }

    pub fn get_chunk_at(&self, position: &ChunkPosition) -> Result<&ModifiedChunk, ModifiedWorldError> {
        self.chunks.get(position).ok_or(ModifiedWorldError::ValeurInvalide(*position))
    }

    pub fn get_chunk_at_mut(&mut self, position: &ChunkPosition) -> Result<&mut ModifiedChunk, String> {
        self.chunks
            .get_mut(position)
            .ok_or_else(|| format!("Le chunk ({:?}) n'existe pas ou n'a pas été modifié", position))
    }

    pub fn get_block_at(&self, position: &WorldPosition) -> Option<&BlockInstance> {
        let (chunk_position, block_position) = position.into();
        let chunk = self.chunks.get(&chunk_position)?;
        chunk.get_block_at(&block_position)
    }

    pub fn set_block_at(&mut self, position: &WorldPosition, block: BlockInstance) {
        let (chunk_position, block_position) = position.into();

        let chunk = self.chunks.entry(chunk_position).or_default();
        chunk.set_block_at(&block_position, block);
    }

    pub fn remove_block_at(&mut self, position: &WorldPosition) -> Option<BlockInstance> {
        let (chunk_position, block_position) = position.into();
        let chunk = self.chunks.get_mut(&chunk_position)?;
        chunk.remove_block_at(&block_position)
    }

    pub const fn chunks(&self) -> &FxHashMap<ChunkPosition, ModifiedChunk> {
        &self.chunks
    }

    pub fn into_chunks(self) -> FxHashMap<ChunkPosition, ModifiedChunk> {
        self.chunks
    }
}
