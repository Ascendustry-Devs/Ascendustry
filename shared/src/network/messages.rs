use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: u8 = 1;
pub const MAX_PAQUET_SIZE: usize = 65536;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PublicPlayerData {
    player_id: u64,
    position: Position,
    rotation: Rotation,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PrivatePlayerData {}

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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContenuPaquet {
    DonneesConnexion { version: u8, username: String },
    Confirmation { player_id: u64, server_time: u64 },
    PlayerTransformation { data: PlayerTransformation },
    MultiplePlayerTransformation { data: Vec<PlayerTransformation> },
    GuardCorrection { data: Vec<PlayerTransformation> },
    DonneesMonde { chunks: Vec<ChunkData> },
    ServerSeed { seed: u32 },
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    SetBlock { x: i32, y: i32, z: i32, block_id: u32 },
    GamemodeChange { player_id: u64, gamemode: PlayerGameMode },
}

#[derive(Clone, Copy, Serialize, Debug, Deserialize, PartialEq)]
pub enum PlayerGameMode {
    // God,
    Spectator,
    Survival,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Rotation {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerTransformation {
    pub player_id: u64,
    pub position: Position,
    pub rotation: Rotation,
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
    pub fn new(type_paquet: TypePaquet, contenu: ContenuPaquet) -> Self {
        Self { type_paquet, contenu }
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize packet")
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

pub fn create_handshake(username: String) -> Paquet {
    Paquet::new(
        TypePaquet::Handshake,
        ContenuPaquet::DonneesConnexion {
            version: CURRENT_VERSION,
            username,
        },
    )
}

pub fn create_handshake_ack(player_id: u64, server_time: u64) -> Paquet {
    Paquet::new(TypePaquet::HandshakeAck, ContenuPaquet::Confirmation { player_id, server_time })
}

pub fn create_player_update(player_id: u64, x: f32, y: f32, z: f32, rx: f32, ry: f32) -> Paquet {
    Paquet::new(
        TypePaquet::PlayerTransformation,
        ContenuPaquet::PlayerTransformation {
            data: PlayerTransformation {
                player_id: player_id,
                position: Position { x, y, z },
                rotation: Rotation { x: rx, y: ry },
            },
        },
    )
}

pub fn new_server_seed_paquet(seed: u32) -> Paquet {
    Paquet::new(TypePaquet::ServerSeed, ContenuPaquet::ServerSeed { seed })
}

pub fn new_ping_paquet(timestamp: u64) -> Paquet {
    Paquet::new(TypePaquet::Ping, ContenuPaquet::Ping { timestamp })
}

pub fn new_pong_paquet(timestamp: u64) -> Paquet {
    Paquet::new(TypePaquet::Pong, ContenuPaquet::Pong { timestamp })
}

pub fn new_set_block_paquet(x: i32, y: i32, z: i32, block_id: u32) -> Paquet {
    Paquet::new(TypePaquet::SetBlock, ContenuPaquet::SetBlock { x, y, z, block_id })
}

pub fn new_gamemode_change_paquet(player_id: u64, gamemode: PlayerGameMode) -> Paquet {
    Paquet::new(TypePaquet::GamemodeChange, ContenuPaquet::GamemodeChange { player_id, gamemode })
}
