use crate::client::ClientSession;
use crate::game::ProductionHandler;
use crate::persistence::PersistenceService;
use crate::state::AppState;
#[cfg(feature = "tui")]
use crate::tui::bridge::TuiBridge;

#[cfg(feature = "tui")]
pub type BridgeOption = Option<TuiBridge>;
#[cfg(not(feature = "tui"))]
pub type BridgeOption = Option<()>;
use anyhow::Result;
use game::constants::GUARD_CYCLE_INTERVAL_MS;
use network::crypto::generate_server_id;
use network::messages::BroadcastMessage;
use project_core::log_err_server;
use project_core::log_server;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot};

pub struct Server {
    listener: TcpListener,
    pub state: Arc<AppState>,
    persistence: Arc<PersistenceService>,
    broadcaster: broadcast::Sender<BroadcastMessage>,
    next_id: AtomicU64,
    #[cfg(feature = "tui")]
    bridge: Option<TuiBridge>,
    active_sessions: Arc<Mutex<HashMap<u64, oneshot::Sender<()>>>>,
}

impl Server {
    pub async fn new(address: &str, save_path: &str, bridge: BridgeOption) -> Result<Self> {
        let listener = TcpListener::bind(address).await?;
        let state = Arc::new(AppState::new());
        let persistence = Arc::new(PersistenceService::new(save_path));
        if let Ok(Some(data)) = persistence.load() {
            log_server!("Sauvegarde trouvée, restauration du monde.");
            state.import_save(data).await;
        } else {
            log_server!("Aucune sauvegarde trouvée, création d'un nouveau monde.");
            state.init_random_seed().await;
        }
        let (broadcaster, _) = crate::broadcast::channel();
        Ok(Self {
            listener,
            state,
            persistence,
            broadcaster,
            next_id: AtomicU64::new(0),
            #[cfg(feature = "tui")]
            bridge: bridge,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
    }

    pub async fn save(&self) -> Result<()> {
        let data = self.state.export_save().await;
        self.persistence.save(&data)
    }

    pub async fn run(&self) -> Result<()> {
        log_server!("Serveur: démarre à l'adresse {}.", self.listener.local_addr()?);

        let state = Arc::clone(&self.state);
        let bc = self.broadcaster.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(GUARD_CYCLE_INTERVAL_MS));
            loop {
                interval.tick().await;
                state.run_guard_cycle(&bc).await;
            }
        });
        let state = Arc::clone(&self.state);
        let persistence = self.persistence.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_mins(5)); // 5min
            loop {
                interval.tick().await;
                let data = state.export_save().await;
                if let Err(e) = persistence.save(&data) {
                    log_err_server!("Auto-save échoué : {}", e);
                } else {
                    log_server!("Auto-save effectué.");
                }
            }
        });

        #[cfg(feature = "tui")]
        if let Some(bridge) = self.bridge.as_ref() {
            let state = Arc::clone(&self.state);
            let b = bridge.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(3));
                loop {
                    interval.tick().await;
                    b.sync_from_appstate(&state).await;
                }
            });
        }

        loop {
            let (stream, addr) = self.listener.accept().await?;
            log_server!("Serveur: connexion de l'adresse {}.", addr);

            let player_id = self.next_id.fetch_add(1, Ordering::SeqCst);
            let server_id = generate_server_id();
            log_server!("Joueur {}: connexion (ID serveur: {:02x?}).", player_id, server_id);

            let handler = ProductionHandler;

            let (kick_tx, kick_rx) = oneshot::channel();
            self.active_sessions.lock().unwrap().insert(player_id, kick_tx);
            let session = ClientSession::new(
                player_id,
                0,
                server_id,
                handler,
                Arc::clone(&self.state),
                self.broadcaster.clone(),
                Arc::clone(&self.persistence),
                kick_rx,
            );
            let sessions = Arc::clone(&self.active_sessions);

            tokio::spawn(async move {
                if let Err(e) = session.run(stream).await {
                    log_err_server!("Échec du traitement du client.\nErreur : {}", e);
                }
                sessions.lock().unwrap().remove(&player_id);
            });
        }
    }
    pub async fn kick_player(&self, id: &u64, reason: &str) -> bool {
        let mut sessions = self.active_sessions.lock().unwrap();
        if let Some(tx) = sessions.remove(id) {
            let _ = tx.send(());
            self.state.kick_player(id, reason).await;
            true
        } else {
            log_server!("Joueur {} non trouvé dans les sessions actives.", id);
            false
        }
    }
}
