use crate::world::data::block::BlockInstance;
use crate::world::data::chunk::global_position_to_chunk_pos;
use crate::world::data::chunk::IntraChunkCoords;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

#[derive(Debug)]
pub enum ModifiedWorldError {
    ValeurInvalide(i32, i32, i32),
}

impl fmt::Display for ModifiedWorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModifiedWorldError::ValeurInvalide(cx, cy, cz) => {
                write!(f, "Le chunk ({}, {}, {}) n'existe pas ou n'a pas été modifié", *cx, *cy, *cz)
            }
        }
    }
}

pub struct ModifiedChunk {
    blocks: Vec<(IntraChunkCoords, BlockInstance)>,
    index: HashMap<IntraChunkCoords, usize>, // Table de correspondance pour une recherche en quasiment O(1) (t'as vu ça @Strachy c'est giga smart)
}

impl ModifiedChunk {
    pub fn new() -> Self {
        Self {
            blocks: vec![],
            index: HashMap::new(),
        }
    }

    pub fn get_block_at(&self, coords: &IntraChunkCoords) -> Option<&BlockInstance> {
        self.index.get(coords).and_then(|&i| self.blocks.get(i)).map(|(_, block)| block)
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
}

pub struct ModifiedWorld {
    chunks: HashMap<(i32, i32, i32), ModifiedChunk>,
}

impl ModifiedWorld {
    pub fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    pub fn get_chunk_at(&self, cx: i32, cy: i32, cz: i32) -> Result<&ModifiedChunk, ModifiedWorldError> {
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => Ok(chunk),
            None => Err(ModifiedWorldError::ValeurInvalide(cx, cy, cz)),
        }
    }

    // Inutile pour l'instant
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

    /// Modifie ou ajoute le bloc
    pub fn set_block_at(&mut self, gx: i32, gy: i32, gz: i32, block: BlockInstance) {
        let (chunk_pos, intra_coords) = global_position_to_chunk_pos(gx, gy, gz);

        // Crée le chunk de modif s'il n'existe pas
        let chunk = self.chunks.entry(chunk_pos).or_insert_with(|| ModifiedChunk::new());
        chunk.set_block_at(intra_coords, block);
    }

    // Supprime le bloc et renvoie Option<BlockInstance>
    pub fn remove_block_at(&mut self, gx: i32, gy: i32, gz: i32) -> Option<BlockInstance> {
        let (chunk_pos, intra_coords) = global_position_to_chunk_pos(gx, gy, gz);
        let chunk = self.chunks.get_mut(&chunk_pos)?;
        chunk.remove_block_at(&intra_coords)
    }

    pub fn retain_chunks(&mut self, keep: &HashSet<(i32, i32, i32)>) {
        self.chunks.retain(|key, _| keep.contains(key));
    }
}
