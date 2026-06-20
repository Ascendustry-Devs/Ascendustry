pub mod broadcast;
pub mod client;
pub mod config;
pub mod game;
pub mod identity;
pub mod metrics;
pub mod network_server;
pub mod persistence;
pub mod player;
pub mod rate_limiter;
pub mod server;
pub mod state;
pub mod world;

pub mod tui;

#[cfg(feature = "tui")]
#[cfg(test)]
mod tests;

use anyhow::Result;
use network::DEFAULT_SERVER_ADDRESS;
use project_core::log_server;
use server::Server;

pub use config::ServerConfig;

pub async fn run_server(save_path: &str) -> Result<()> {
    log_server!("Serveur: lancement.");
    let x = String::from(DEFAULT_SERVER_ADDRESS);
    let server = Server::new(&x, save_path, None, ServerConfig::default()).await?;
    server.run().await
}
