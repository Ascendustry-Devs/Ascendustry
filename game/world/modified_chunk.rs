use std::collections::HashMap;
use std::fmt;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::world::data::block::BlockInstance;
use crate::world::data::chunk::{global_position_to_chunk_pos, IntraChunkCoords};

#[derive(Debug)]
pub enum ModifiedWorldError {
    ValeurInvalide(i32, i32, i32),
}

impl fmt::Display for ModifiedWorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValeurInvalide(cx, cy, cz) => {
                write!(f, "Le chunk ({}, {}, {}) n'existe pas ou n'a pas été modifié", *cx, *cy, *cz)
            }
        }
    }
}

pub struct ModifiedChunk {
    blocks: Vec<(IntraChunkCoords, BlockInstance)>,
    index: FxHashMap<IntraChunkCoords, usize>,
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

    pub fn get_block_at(&self, coords: &IntraChunkCoords) -> Option<&BlockInstance> {
        self.index
            .get(coords)
            .and_then(|&i| self.blocks.get(i))
            .map(|(_, block)| block)
    }

    pub fn set_block_at(&mut self, coords: IntraChunkCoords, block: BlockInstance) {
        if let Some(&i) = self.index.get(&coords) {
            self.blocks[i].1 = block;
        } else {
            let i = self.blocks.len();
            self.blocks.push((coords, block));
            self.index.insert(coords, i);
        }
    }

    pub fn remove_block_at(&mut self, coords: &IntraChunkCoords) -> Option<BlockInstance> {
        let i = self.index.remove(coords)?;
        let (_, removed_block) = self.blocks.swap_remove(i);

        if i < self.blocks.len() {
            let moved_coords = self.blocks[i].0;
            self.index.insert(moved_coords, i);
        }

        Some(removed_block)
    }
    pub fn blocks(&self) -> &[(IntraChunkCoords, BlockInstance)] {
        &self.blocks
    }

    pub fn into_blocks(self) -> Vec<(IntraChunkCoords, BlockInstance)> {
        self.blocks
    }

    pub fn from_blocks(blocks: Vec<(IntraChunkCoords, BlockInstance)>) -> Self {
        let mut index = HashMap::with_capacity_and_hasher(blocks.len(), FxBuildHasher);
        for (i, (coords, _)) in blocks.iter().enumerate() {
            index.insert(*coords, i);
        }
        Self { blocks, index }
    }
}

pub struct ModifiedWorld {
    pub chunks: FxHashMap<(i32, i32, i32), ModifiedChunk>,
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

    pub fn get_chunk_at(&self, cx: i32, cy: i32, cz: i32) -> Result<&ModifiedChunk, ModifiedWorldError> {
        self.chunks
            .get(&(cx, cy, cz))
            .ok_or(ModifiedWorldError::ValeurInvalide(cx, cy, cz))
    }

    pub fn get_chunk_at_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Result<&mut ModifiedChunk, String> {
        self.chunks
            .get_mut(&(cx, cy, cz))
            .ok_or_else(|| format!("Le chunk ({}, {}, {}) n'existe pas ou n'a pas été modifié", cx, cy, cz))
    }

    pub fn get_block_at(&self, gx: i32, gy: i32, gz: i32) -> Option<&BlockInstance> {
        let (chunk_pos, intra_coords) = global_position_to_chunk_pos(gx, gy, gz);
        let chunk = self.chunks.get(&chunk_pos)?;
        chunk.get_block_at(&intra_coords)
    }

    pub fn set_block_at(&mut self, gx: i32, gy: i32, gz: i32, block: BlockInstance) {
        let (chunk_pos, intra_coords) = global_position_to_chunk_pos(gx, gy, gz);

        let chunk = self.chunks.entry(chunk_pos).or_default();
        chunk.set_block_at(intra_coords, block);
    }

    pub fn remove_block_at(&mut self, gx: i32, gy: i32, gz: i32) -> Option<BlockInstance> {
        let (chunk_pos, intra_coords) = global_position_to_chunk_pos(gx, gy, gz);
        let chunk = self.chunks.get_mut(&chunk_pos)?;
        chunk.remove_block_at(&intra_coords)
    }

    pub const fn chunks(&self) -> &FxHashMap<(i32, i32, i32), ModifiedChunk> {
        &self.chunks
    }

    pub fn into_chunks(self) -> FxHashMap<(i32, i32, i32), ModifiedChunk> {
        self.chunks
    }
}
