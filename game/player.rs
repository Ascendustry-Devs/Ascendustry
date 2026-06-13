use serde::{Deserialize, Serialize};

use crate::types::{Position, Rotation};

#[derive(Clone, Copy, Serialize, Debug, Deserialize, PartialEq)]
pub enum PlayerGameMode {
    Spectator,
    Survival,
}

impl std::fmt::Display for PlayerGameMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            PlayerGameMode::Survival => "PlayerGameMode::Survival",
            PlayerGameMode::Spectator => "PlayerGameMode::Spectator",
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerTransformation {
    pub player_id: u64,
    pub position: Position,
    pub rotation: Rotation,
}
