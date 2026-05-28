use client::run_client;
use network::DEFAULT_SERVER_ADDRESS;
use server::run_server;
use tokio::runtime::Runtime;

pub enum LaunchMode {
    Singleplayer(String),
    Multiplayer(String),
}

pub fn set_play_mode(runtime: &Runtime, mode: LaunchMode) {
    match mode {
        LaunchMode::Singleplayer(save_path) => start_singleplayer(runtime, &save_path),
        LaunchMode::Multiplayer(address) => start_multiplayer(&address),
    }
}

pub fn start_singleplayer(runtime: &Runtime, save_path: &str) {
    let save_path = save_path.to_string();
    runtime.spawn(async move {
        if let Err(e) = run_server(&save_path).await {
            eprintln!("Erreur: {}", e);
        }
    });

    run_client(DEFAULT_SERVER_ADDRESS);
}

pub fn start_multiplayer(address: &str) {
    run_client(address);
}
