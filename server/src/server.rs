use crate::client::ClientSession;
use crate::game::{PacketHandler, ProductionHandler};
use crate::state::AppState;
use anyhow::Result;
use satiscore::constants::GUARD_CYCLE_INTERVAL_MS;
use satiscore::log_err_server;
use satiscore::log_server;
use satiscore::network::crypto::generate_server_id;
use satiscore::network::messages::Paquet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub struct Server {
    listener: TcpListener,
    state: Arc<AppState>,
    broadcaster: broadcast::Sender<Paquet>,
    next_player_id: AtomicU64,
}

impl Server {
    pub async fn new(address: &str) -> Result<Self> {
        let listener = TcpListener::bind(address).await?;
        let state = Arc::new(AppState::new());
        let (broadcaster, _) = crate::broadcast::channel();
        Ok(Self {
            listener,
            state,
            broadcaster,
            next_player_id: AtomicU64::new(1),
        })
    }

    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
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

        loop {
            let (stream, addr) = self.listener.accept().await?;
            log_server!("Serveur: connexion de l'adresse {}.", addr);

            let player_id = self.next_player_id.fetch_add(1, Ordering::SeqCst);
            let server_id = generate_server_id();
            log_server!("Joueur {}: connexion (ID serveur: {:02x?}).", player_id, server_id);

            let handler: Box<dyn PacketHandler> = Box::new(ProductionHandler);
            let session = ClientSession::new(player_id, server_id, handler, Arc::clone(&self.state), self.broadcaster.clone());

            tokio::spawn(async move {
                if let Err(e) = session.run(stream).await {
                    log_err_server!("Échec du traitement du client.\nErreur : {}", e);
                }
            });
        }
    }
}
