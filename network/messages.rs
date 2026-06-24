use game::inventory::{Inventory, SlotData};
use game::player::{PlayerGameMode, PlayerTransformation};
use game::types::{Position, Rotation};
use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: u8 = 1;
pub const MAX_PAQUET_SIZE: usize = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BroadcastMessage {
    All(Paquet),
    AllExcept { player_id: u64, paquet: Paquet },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypePaquet {
    Handshake,
    HandshakeAck,
    PlayerTransformation,
    MultiplePlayerTransformation,
    GuardCorrection,
    ServerSeed,
    WorldData,
    MovePlayer,
    Ping,
    Pong,
    SetBlock,
    GamemodeChange,
    SaveRequest,
    Kick,
    ClientIdentity,
    InventoryUpdate,
    InventorySet,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContenuPaquet {
    DonneesConnexion {
        version: u8,
        username: String,
        player_unique_id: u64,
    },
    Confirmation {
        player_id: u64,
        is_player_id_correct: bool,
        server_time: u64,
    },
    PlayerTransformation {
        data: PlayerTransformation,
    },
    MultiplePlayerTransformation {
        data: Vec<PlayerTransformation>,
    },
    GuardCorrection {
        data: Vec<PlayerTransformation>,
    },
    DonneesMonde {
        chunks: Vec<ChunkData>,
    },
    ServerSeed {
        seed: u32,
    },
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },
    SetBlock {
        x: i32,
        y: i32,
        z: i32,
        block_id: u32,
    },
    GamemodeChange {
        player_id: u64,
        gamemode: PlayerGameMode,
    },
    SaveRequest,
    Kick {
        reason: String,
    },
    ClientIdentity {
        player_id: u64,
        username: String,
    },
    /// Modification sur l'inventaire (serveur => client et client => serveur)
    InventoryUpdate {
        player_id: u64,
        modified_slots: Vec<SlotData>, // Slots modifié seulement
    },
    /// Inventaire complet du serveur => client
    InventorySet {
        inventory: Inventory, // Inventaire complet
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChunkData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Paquet {
    pub type_paquet: TypePaquet,
    pub contenu: ContenuPaquet,
}

impl Paquet {
    pub const fn new(type_paquet: TypePaquet, contenu: ContenuPaquet) -> Self {
        Self { type_paquet, contenu }
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize packet")
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

pub const fn create_handshake(username: String, player_unique_id: u64) -> Paquet {
    Paquet::new(
        TypePaquet::Handshake,
        ContenuPaquet::DonneesConnexion {
            version: CURRENT_VERSION,
            username,
            player_unique_id,
        },
    )
}

pub const fn create_handshake_ack(player_id: u64, server_time: u64, is_player_id_correct: bool) -> Paquet {
    Paquet::new(
        TypePaquet::HandshakeAck,
        ContenuPaquet::Confirmation {
            player_id,
            server_time,
            is_player_id_correct,
        },
    )
}

pub const fn create_player_update(player_id: u64, x: f32, y: f32, z: f32, rx: f32, ry: f32) -> Paquet {
    Paquet::new(
        TypePaquet::PlayerTransformation,
        ContenuPaquet::PlayerTransformation {
            data: PlayerTransformation {
                player_id,
                position: Position { x, y, z },
                rotation: Rotation { x: rx, y: ry },
            },
        },
    )
}

pub const fn new_server_seed_paquet(seed: u32) -> Paquet {
    Paquet::new(TypePaquet::ServerSeed, ContenuPaquet::ServerSeed { seed })
}

pub const fn new_ping_paquet(timestamp: u64) -> Paquet {
    Paquet::new(TypePaquet::Ping, ContenuPaquet::Ping { timestamp })
}

pub const fn new_pong_paquet(timestamp: u64) -> Paquet {
    Paquet::new(TypePaquet::Pong, ContenuPaquet::Pong { timestamp })
}

pub const fn new_set_block_paquet(x: i32, y: i32, z: i32, block_id: u32) -> Paquet {
    Paquet::new(TypePaquet::SetBlock, ContenuPaquet::SetBlock { x, y, z, block_id })
}

pub const fn new_gamemode_change_paquet(player_id: u64, gamemode: PlayerGameMode) -> Paquet {
    Paquet::new(
        TypePaquet::GamemodeChange,
        ContenuPaquet::GamemodeChange { player_id, gamemode },
    )
}

pub const fn new_save_request_paquet() -> Paquet {
    Paquet::new(TypePaquet::SaveRequest, ContenuPaquet::SaveRequest)
}

pub const fn new_kick_paquet(reason: String) -> Paquet {
    Paquet::new(TypePaquet::Kick, ContenuPaquet::Kick { reason })
}

pub const fn new_client_identity_paquet(player_id: u64, username: String) -> Paquet {
    Paquet::new(
        TypePaquet::ClientIdentity,
        ContenuPaquet::ClientIdentity { player_id, username },
    )
}
pub const fn new_inventory_update_paquet(player_id: u64, slots: Vec<SlotData>) -> Paquet {
    Paquet::new(
        TypePaquet::InventoryUpdate,
        ContenuPaquet::InventoryUpdate {
            player_id,
            modified_slots: slots,
        },
    )
}
