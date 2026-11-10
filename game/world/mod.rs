use std::sync::{Arc, RwLock};

use project_core::log_warn_server;

use crate::{
    inventory::ItemManager,
    world::{
        chunk_position::ChunkPosition,
        data::{
            block::{BlockInstance, BlockManager},
            chunk::{Chunk, ChunkData},
        },
        generation::chunk_generator::ChunkGenerator,
        local_position::LocalPosition,
        world_position::WorldPosition,
    },
};

pub mod chunk_position;
pub mod data;
pub mod generation;
pub mod local_position;
pub mod modified_chunk;
pub mod raycast;
pub mod scheduling;
pub mod world_position;

pub trait ChunkStorage {
    type Value: ChunkAccess + ChunkModification + 'static;
    fn get(&self, position: &ChunkPosition) -> Option<&Self::Value>;
    fn get_mut(&mut self, position: &ChunkPosition) -> Option<&mut Self::Value>;
    fn contains_key(&self, coords: &ChunkPosition) -> bool;
    fn extend<I: Iterator<Item = (ChunkPosition, Self::Value)>>(&mut self, iter: I);
    fn retain<F: FnMut(&ChunkPosition, &mut Self::Value) -> bool>(&mut self, f: F);
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn clear(&mut self);
}

pub trait World {
    type Storage: ChunkStorage + 'static;

    fn chunks(&self) -> &Self::Storage;
    fn chunks_mut(&mut self) -> &mut Self::Storage;

    fn get_chunk(&self, position: &ChunkPosition) -> Option<&Chunk> {
        self.chunks().get(position).map(ChunkAccess::get_chunk)
    }

    fn get_chunk_mut(&mut self, position: &ChunkPosition) -> Option<&mut Chunk> {
        self.chunks_mut().get_mut(position).map(ChunkAccess::get_chunk_mut)
    }

    fn seed(&self) -> u32;
    fn chunk_generator(&self) -> &ChunkGenerator;
    fn block_manager(&self) -> &Arc<RwLock<BlockManager>>;
    fn item_manager(&self) -> &Arc<RwLock<ItemManager>>;

    fn set_block(&mut self, position: &WorldPosition, block: BlockInstance) -> Result<(), ChunkPosition> {
        let (chunk_position, local_position): (ChunkPosition, LocalPosition) = position.into();

        if let Some(chunk) = self.chunks_mut().get_mut(&chunk_position) {
            chunk.set_block(&local_position, block);
            Ok(())
        } else {
            log_warn_server!("Tentative de modification d'un chunk non chargé : {:?}", chunk_position);
            Err(chunk_position)
        }
    }

    fn get_block(&self, position: &WorldPosition) -> Option<BlockInstance> {
        let (chunk_position, local_position): (ChunkPosition, LocalPosition) = position.into();

        if let Some(chunk) = self.get_chunk(&chunk_position) {
            Some(chunk.get_block_from_local_xyz(&local_position))
        } else {
            log_warn_server!("Tentative d'accès à un chunk non chargé : {:?}", chunk_position);
            None
        }
    }
}

pub trait ChunkAccess {
    fn get_chunk(&self) -> &Chunk;
    fn get_chunk_mut(&mut self) -> &mut Chunk;
}

pub trait ChunkModification {
    fn set_block(&mut self, position: &LocalPosition, block: BlockInstance) -> Option<BlockInstance>;
}

impl ChunkAccess for ChunkData {
    fn get_chunk(&self) -> &Chunk {
        &self.chunk
    }
    fn get_chunk_mut(&mut self) -> &mut Chunk {
        Arc::make_mut(&mut self.chunk)
    }
}

impl ChunkModification for ChunkData {
    fn set_block(&mut self, position: &LocalPosition, block: BlockInstance) -> Option<BlockInstance> {
        if self.chunk.get_block_from_local_xyz(position) == block {
            return None;
        }
        self.set_dirty();
        Arc::make_mut(&mut self.chunk).set_block_from_local_xyz(position, block)
    }
}
