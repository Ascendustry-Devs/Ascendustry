use crate::client::ClientSession;
use crate::game::{PacketHandler, ProductionHandler};
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub struct Server {
    listener: TcpListener,
    pub state: Arc<AppState>,
    persistence: Arc<PersistenceService>,
    broadcaster: broadcast::Sender<BroadcastMessage>,
    next_player_id: AtomicU64,
    #[cfg(feature = "tui")]
    bridge: Option<TuiBridge>,
}

impl Server {
    pub async fn new(address: &str, save_path: &str, bridge: BridgeOption) -> Result<Self> {
        let listener = TcpListener::bind(address).await?;
        let state = Arc::new(AppState::new());
        let persistence = Arc::new(PersistenceService::new(save_path));
        if let Ok(Some(data)) = persistence.load() {
            log_server!("Sauvegarde trouvée, restauration du monde.");
            state.import_save(data);
        } else {
            log_server!("Aucune sauvegarde trouvée, création d'un nouveau monde.");
            state.init_random_seed();
        }
        let (broadcaster, _) = crate::broadcast::channel();
        Ok(Self {
            listener,
            state,
            persistence,
            broadcaster,
            next_player_id: AtomicU64::new(1),
            #[cfg(feature = "tui")]
            bridge: bridge,
        })
    }

    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
    }

    pub fn save(&self) -> Result<()> {
        let data = self.state.export_save();
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
                state.run_guard_cycle(&bc);
            }
        });
        let state = Arc::clone(&self.state);
        let persistence = self.persistence.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_mins(5)); // 5min
            loop {
                interval.tick().await;
                let data = state.export_save();
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
                    b.sync_from_appstate(&state);
                }
            });
        }

        loop {
            let (stream, addr) = self.listener.accept().await?;
            log_server!("Serveur: connexion de l'adresse {}.", addr);

            let player_id = self.next_player_id.fetch_add(1, Ordering::SeqCst);
            let server_id = generate_server_id();
            log_server!("Joueur {}: connexion (ID serveur: {:02x?}).", player_id, server_id);

            let handler: Box<dyn PacketHandler> = Box::new(ProductionHandler);
            let session = ClientSession::new(
                player_id,
                server_id,
                handler,
                Arc::clone(&self.state),
                self.broadcaster.clone(),
                Arc::clone(&self.persistence),
            );

            tokio::spawn(async move {
                if let Err(e) = session.run(stream).await {
                    log_err_server!("Échec du traitement du client.\nErreur : {}", e);
                }
            });
        }
    }
}
