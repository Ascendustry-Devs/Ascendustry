use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub struct PlayerInfo {
    pub id: u64,
    pub username: String,
    pub position: (f32, f32, f32),
    pub gamemode: String,
}

pub struct TuiState {
    pub players: Vec<PlayerInfo>,
    pub logs: Vec<String>,
    pub address: String,
    pub start_time: std::time::Instant,
    pub seed: u32,
    pub chunk_count: usize,
    pub modified_count: usize,
    pub connected_player_count: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            players: Vec::new(),
            logs: Vec::new(),
            address: String::new(),
            start_time: std::time::Instant::now(),
            seed: 0,
            chunk_count: 0,
            modified_count: 0,
            connected_player_count: 0,
        }
    }
}

pub enum TuiCommand {
    Shutdown,
    Save,
    Kick(u64),
    Log(String),
}

#[derive(Clone)]
pub struct TuiBridge {
    pub state: Arc<Mutex<TuiState>>,
    pub command_tx: mpsc::UnboundedSender<TuiCommand>,
}

impl TuiBridge {
    pub fn new(state: Arc<Mutex<TuiState>>, command_tx: mpsc::UnboundedSender<TuiCommand>) -> Self {
        Self { state, command_tx }
    }

    pub fn set_address(&self, address: &str) {
        self.state.lock().unwrap().address = address.to_string();
    }

    pub async fn sync_from_appstate(&self, app_state: &crate::state::AppState) {
        let seed = app_state.get_seed().await;
        let chunk_count = app_state.get_chunk_count().await;
        let modified_count = app_state.get_modified_count().await;
        let players = app_state.get_all_players_vec().await.unwrap_or_default();
        let connected_count = players.len();

        let mut s = self.state.lock().unwrap();
        s.seed = seed;
        s.chunk_count = chunk_count;
        s.modified_count = modified_count;
        s.connected_player_count = connected_count;
        s.players = players
            .iter()
            .map(|p| PlayerInfo {
                id: p.id,
                username: p.username.clone(),
                position: (p.position.x, p.position.y, p.position.z),
                gamemode: format!("{:?}", p.gamemode),
            })
            .collect();
    }
}
