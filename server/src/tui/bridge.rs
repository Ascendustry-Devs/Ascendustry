use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

use game::constants::GUARD_CYCLE_INTERVAL_MS;

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

    // Santé / métriques
    pub health_score: u8,
    pub packets_per_second: f64,
    pub guard_cycle_load_pct: f64,
    pub guard_cycle_avg_ms: f64,
    pub packets_rejected: u64,
    pub total_connections: u64,

    // Valeurs précédentes pour le calcul des deltas
    prev_packets_received: u64,
    prev_packets_rejected: u64,
    prev_guard_count: u64,
    prev_guard_cumul_ns: u64,
    prev_sync_time: Instant,
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
            health_score: 20,
            packets_per_second: 0.0,
            guard_cycle_load_pct: 0.0,
            guard_cycle_avg_ms: 0.0,
            packets_rejected: 0,
            total_connections: 0,
            prev_packets_received: 0,
            prev_packets_rejected: 0,
            prev_guard_count: 0,
            prev_guard_cumul_ns: 0,
            prev_sync_time: Instant::now(),
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

        // Lecture des compteurs atomiques
        let metrics = &app_state.metrics;
        let packets_rcvd = metrics.packets_received.load(Ordering::Relaxed);
        let packets_rej = metrics.packets_rejected.load(Ordering::Relaxed);
        let guard_count = metrics.guard_cycle_count.load(Ordering::Relaxed);
        let guard_cumul_ns = metrics.guard_cycle_cumul_ns.load(Ordering::Relaxed);
        let total_conn = metrics.total_connections.load(Ordering::Relaxed);

        let mut s = self.state.lock().unwrap();
        s.seed = seed;
        s.chunk_count = chunk_count;
        s.modified_count = modified_count;
        s.connected_player_count = connected_count;
        s.total_connections = total_conn;
        s.players = players
            .iter()
            .map(|p| PlayerInfo {
                id: p.id,
                username: p.username.clone(),
                position: (p.position.x, p.position.y, p.position.z),
                gamemode: format!("{:?}", p.gamemode),
            })
            .collect();

        // Calcul des débits à partir des deltas
        let dt = s.prev_sync_time.elapsed().as_secs_f64().max(0.001);
        s.packets_per_second = packets_rcvd.saturating_sub(s.prev_packets_received) as f64 / dt;
        s.packets_rejected = packets_rej.saturating_sub(s.prev_packets_rejected);

        if guard_count > s.prev_guard_count {
            let avg_ns = (guard_cumul_ns - s.prev_guard_cumul_ns) / (guard_count - s.prev_guard_count);
            s.guard_cycle_avg_ms = avg_ns as f64 / 1_000_000.0;
            s.guard_cycle_load_pct = avg_ns as f64 / (GUARD_CYCLE_INTERVAL_MS as f64 * 1_000_000.0) * 100.0;
        }

        // Calcul du score de santé (0-20)
        let mut health = 20u8;
        let half_interval = GUARD_CYCLE_INTERVAL_MS as f64 * 0.5;
        if s.guard_cycle_avg_ms > half_interval {
            health = health.saturating_sub(((s.guard_cycle_avg_ms - half_interval) / GUARD_CYCLE_INTERVAL_MS as f64 * 10.0) as u8);
        }
        health = health.saturating_sub((s.packets_rejected as f64 * 0.5).min(5.0) as u8);
        if s.chunk_count > 100_000 {
            health = health.saturating_sub((((s.chunk_count - 100_000) as f64 / 100_000.0) * 3.0).min(3.0) as u8);
        }
        s.health_score = health.min(20);

        // Mise à jour des valeurs précédentes
        s.prev_packets_received = packets_rcvd;
        s.prev_packets_rejected = packets_rej;
        s.prev_guard_count = guard_count;
        s.prev_guard_cumul_ns = guard_cumul_ns;
        s.prev_sync_time = Instant::now();
    }
}
