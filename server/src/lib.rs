pub mod broadcast;
pub mod client;
pub mod game;
pub mod network;
pub mod player;
pub mod server;
pub mod state;
pub mod world;

use anyhow::Result;
use satiscore::log_server;
use satiscore::network::DEFAULT_SERVER_ADDRESS;
use server::Server;

pub async fn run_server() -> Result<()> {
    log_server!("Serveur: lancement.");
    let x = String::from(DEFAULT_SERVER_ADDRESS);
    let server = Server::new(&x).await?;
    server.state().init_random_seed();
    server.run().await
}
