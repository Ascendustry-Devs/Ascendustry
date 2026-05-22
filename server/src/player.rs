use satiscore::constants::{SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z};
use satiscore::network::messages::{PlayerGameMode, Position, Rotation};
use satiscore::world::data::chunk::CHUNK_SIZE_F;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub username: String,
    pub position: Position,
    pub rotation: Rotation,
    pub gamemode: PlayerGameMode,
    pub last_valid_position: Position,
    pub last_valid_rotation: Rotation,
}

pub struct PlayerRegistry {
    players: HashMap<u64, Player>,
    player_chunks: HashMap<u64, HashSet<(i32, i32, i32)>>,
}

impl PlayerRegistry {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            player_chunks: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: u64, username: String) {
        let position = Position {
            x: SPAWN_POSITION_X,
            y: SPAWN_POSITION_Y,
            z: SPAWN_POSITION_Z,
        };
        let rotation = Rotation { x: 0.0, y: 0.0 };
        let player = Player {
            id,
            username,
            position: position.clone(),
            rotation: rotation.clone(),
            gamemode: PlayerGameMode::Survival,
            last_valid_position: position,
            last_valid_rotation: rotation,
        };
        self.players.insert(id, player);
    }

    pub fn remove(&mut self, id: &u64) -> Option<Player> {
        self.player_chunks.remove(id);
        self.players.remove(id)
    }

    pub fn get(&self, id: &u64) -> Option<&Player> {
        self.players.get(id)
    }
    pub fn get_mut(&mut self, id: &u64) -> Option<&mut Player> {
        self.players.get_mut(id)
    }

    pub fn get_all(&self) -> Option<Vec<Player>> {
        Some(self.players.values().cloned().collect())
    }

    pub fn update_position(&mut self, id: u64, position: Position, rotation: Rotation) -> (i32, i32, i32) {
        if let Some(player) = self.players.get_mut(&id) {
            player.position = position.clone();
            player.rotation = rotation;
        }
        let cx = (position.x / CHUNK_SIZE_F).floor() as i32;
        let cy = (position.y / CHUNK_SIZE_F).floor() as i32;
        let cz = (position.z / CHUNK_SIZE_F).floor() as i32;
        (cx, cy, cz)
    }

    pub fn set_player_chunks(&mut self, id: u64, chunks: HashSet<(i32, i32, i32)>) {
        self.player_chunks.insert(id, chunks);
    }

    pub fn all_required_chunks(&self) -> HashSet<(i32, i32, i32)> {
        self.player_chunks.values().flat_map(|chunks| chunks.iter()).cloned().collect()
    }

    pub fn update_gamemode(&mut self, id: u64, gamemode: PlayerGameMode) {
        if let Some(player) = self.players.get_mut(&id) {
            player.gamemode = gamemode;
        }
    }
    /// Rollback the player's position and rotation
    pub fn reset_to_last_valid_transformation(&mut self, id: u64) {
        if let Some(player) = self.players.get_mut(&id) {
            player.position = player.last_valid_position.clone();
            player.rotation = player.last_valid_rotation.clone();
        }
    }
    pub fn set_last_valid_transformation(&mut self, id: u64, position: Position, rotation: Rotation) {
        if let Some(player) = self.players.get_mut(&id) {
            player.last_valid_position = position.clone();
            player.last_valid_rotation = rotation;
        }
    }
    pub fn get_older_tranformations(&self, id: u64) -> Option<(Position, Rotation)> {
        self.players
            .get(&id)
            .map(|player| (player.last_valid_position.clone(), player.last_valid_rotation.clone()))
    }
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &Player)> {
        self.players.iter()
    }
}

impl Default for PlayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
