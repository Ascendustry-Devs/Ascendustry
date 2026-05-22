use client::run_client;
use satiscore::network::DEFAULT_SERVER_ADDRESS;
use server::run_server;
use tokio::runtime::Runtime;

pub enum LaunchMode {
    Singleplayer,
    Multiplayer(String),
}

pub fn set_play_mode(runtime: &Runtime, mode: LaunchMode) {
    match mode {
        LaunchMode::Singleplayer => start_singleplayer(runtime),
        LaunchMode::Multiplayer(address) => start_multiplayer(&address),
    }
}

pub fn start_singleplayer(runtime: &Runtime) {
    runtime.spawn(async {
        if let Err(e) = run_server().await {
            eprintln!("Erreur: {}", e);
        }
    });

    run_client(DEFAULT_SERVER_ADDRESS);
}

pub fn start_multiplayer(address: &str) {
    run_client(address);
}
