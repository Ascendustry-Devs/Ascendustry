pub mod broadcast;
pub mod client;
pub mod game;
pub mod network_server;
pub mod persistence;
pub mod player;
pub mod server;
pub mod state;
pub mod world;
use anyhow::Result;
use clap::Parser;
use network::DEFAULT_SERVER_ADDRESS;
use satiscore::log_server;
use server::Server;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from(DEFAULT_SERVER_ADDRESS))]
    address: String,
    #[arg(short = 'p', long, default_value = "world/world_1.stf")]
    save_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    log_server!("Serveur: lancement.");
    let args = Args::parse();
    let server = Server::new(&args.address, &args.save_path).await?;
    server.run().await
}
