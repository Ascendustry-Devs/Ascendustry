use network::DEFAULT_SERVER_ADDRESS;
use std::{env::current_exe, process::Command};
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
    runtime.spawn(async move { run_server(&save_path) });

    run_client(DEFAULT_SERVER_ADDRESS);
}

pub fn start_multiplayer(address: &str) {
    run_client(address);
}

pub fn run_client(address: &str) {
    const CLIENT_FILE_NAME: &str = "Ascendustry";
    let client_path = current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(CLIENT_FILE_NAME)))
        .unwrap_or_else(|| CLIENT_FILE_NAME.into());

    let status = Command::new(&client_path).arg("--address").arg(address).status();

    if status.is_err() {
        eprintln!("Le client s'est terminé avec une erreur.\nChemin spécifié: {client_path:?}\nErreur : {status:?}");
    }
}

pub fn run_server(save_path: &str) {
    const SERVER_FILE_NAME: &str = "server";
    let server_path = current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(SERVER_FILE_NAME)))
        .unwrap_or_else(|| SERVER_FILE_NAME.into());

    let status = Command::new(&server_path).arg("-p").arg(save_path).status();

    if status.is_err() {
        eprintln!("Le serveur s'est terminé avec une erreur.\nChemin spécifié: {server_path:?}\nErreur : {status:?}");
    }
}
