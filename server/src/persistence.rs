use core::str;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use game::inventory::Inventory;
use game::player::PlayerGameMode;
use game::types::{Position, Rotation};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use game::world::data::block::BlockInstance;
use game::world::data::chunk::IntraChunkCoords;
use game::world::modified_chunk::ModifiedChunk;
use game::world::modified_chunk::ModifiedWorld;

use crate::identity::IdentityRegistry;
use crate::player::Player;

const SAVE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct SaveChunk {
    pub blocks: Vec<(IntraChunkCoords, BlockInstance)>,
}

#[derive(Serialize, Deserialize)]
pub struct SaveWorld {
    pub chunks: FxHashMap<(i32, i32, i32), SaveChunk>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct PlayerSave {
    pub rotation: Rotation,
    pub position: Position,
    pub gamemode: PlayerGameMode,
    pub inventory: Inventory,
}

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    version: u32,
    pub seed: u32,
    pub world: SaveWorld,
    pub players: Vec<Player>,
    pub identity: IdentityRegistry,
}

impl SaveData {
    pub const fn new(seed: u32, world: SaveWorld, players: Vec<Player>, identity: IdentityRegistry) -> Self {
        Self {
            version: SAVE_VERSION,
            seed,
            world,
            players,
            identity,
        }
    }
}

impl From<&ModifiedWorld> for SaveWorld {
    fn from(world: &ModifiedWorld) -> Self {
        let chunks = world
            .chunks()
            .iter()
            .map(|(&key, chunk)| {
                let blocks = chunk.blocks().to_vec();
                (key, SaveChunk { blocks })
            })
            .collect();
        Self { chunks }
    }
}

impl From<SaveWorld> for ModifiedWorld {
    fn from(world: SaveWorld) -> Self {
        let chunks = world
            .chunks
            .into_iter()
            .map(|(key, chunk)| (key, ModifiedChunk::from(chunk)))
            .collect();
        Self { chunks }
    }
}

impl From<SaveChunk> for ModifiedChunk {
    fn from(chunk: SaveChunk) -> Self {
        Self::from_blocks(chunk.blocks)
    }
}

#[derive(Clone)]
pub struct PersistenceService {
    save_path: PathBuf,
    save_lock: Arc<Mutex<()>>,
}

impl PersistenceService {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            save_path: path.into(),
            save_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn exists(&self) -> bool {
        self.save_path.exists()
    }

    pub async fn save(&self, data: SaveData) -> Result<()> {
        let _lock = self.save_lock.lock().await;
        if let Some(parent) = self.save_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let path = self.save_path.clone();
        let bytes = tokio::task::spawn_blocking(move || bincode::serialize(&data)).await??;
        tokio::fs::write(&path, bytes).await?;
        Ok(())
    }

    pub async fn load(&self) -> Result<Option<SaveData>> {
        if !self.exists() {
            return Ok(None);
        }
        let bytes = tokio::fs::read(&self.save_path).await?;
        let data = tokio::task::spawn_blocking(move || bincode::deserialize(&bytes)).await??;
        Ok(Some(data))
    }
}
