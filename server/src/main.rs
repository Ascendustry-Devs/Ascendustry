//! Serveur Satisfactorio - Point d'entrée.

mod game;
mod network;
mod state;

use crate::game::PacketHandler;
use crate::network::ServerConnection;
use crate::state::GAME_STATE;
use anyhow::Result;
use shared::network::crypto::generate_server_id;
use shared::network::messages::{self, new_server_seed_paquet};
use shared::*;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::*;
use tokio::time::{interval, Duration};

static NEXT_PLAYER_ID: AtomicU64 = AtomicU64::new(1);

async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let player_id = NEXT_PLAYER_ID.fetch_add(1, Ordering::SeqCst);

    let server_id = generate_server_id();
    log_server!("Joueur {}: connexion (ID serveur: {:02x?}).", player_id, server_id);

    let conn = ServerConnection::new(player_id, server_id);
    conn.send_server_id(&mut stream).await?;
    let packet = match conn.receive_packet(&mut stream).await {
        Ok(p) => p,
        Err(e) => {
            log_err_server!("Échec de la réception du paquet.\nErreur : {}", e);
            return Ok(());
        }
    };

    let username = match packet.contenu {
        shared::network::messages::ContenuPaquet::DonneesConnexion { ref username, .. } => username.clone(),
        _ => {
            format!("Player{}", player_id)
        }
    };

    GAME_STATE.add_player(player_id, username.clone());
    log_server!("Joueur {} ({}): ajout à l'état global du serveur", username, player_id);

    // Créer le gestionnaire de paquets pour ce client
    let mut packet_handler = PacketHandler::new();

    packet_handler.handle_packet(packet);
    let ack = messages::create_handshake_ack(player_id, 0);
    if let Err(e) = conn.send_packet(&mut stream, &ack).await {
        log_err_server!("Échec de l'envoi du handshake ack.\nErreur : {}", e);
        return Ok(());
    }

    let seed_packet = new_server_seed_paquet(GAME_STATE.get_seed());
    if let Err(e) = conn.send_packet(&mut stream, &seed_packet).await {
        log_err_server!("Échec de l'envoi de la seed.\nErreur : {}", e);
        return Ok(());
    } else {
        log_server!("Seed envoyée au joueur {} !", player_id);
    }

    let mut interval = interval(Duration::from_millis(1000));

    // Loop de check

    loop {
        tokio::select! {
            result = conn.receive_packet(&mut stream) => {
                match result {
                    Ok(packet) => {
                        if let Some(response) = packet_handler.handle_packet(packet) {
                            conn.send_packet(&mut stream, &response).await?;
                        } else {
                            log_server!("Joueur {}: éjection.", player_id);
                            break;
                        }
                    }
                    Err(e) => {
                        log_err_server!("Échec de la réception du paquet. Erreur : {}", e);
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                match packet_handler.get_players_position_packet() {
                    Ok(packet) => {
                        if let Err(e) = conn.send_packet(&mut stream, &packet).await {
                            log_err_server!("Erreur lors de l'envoi du packet MultiplePlayerTransformation: {}", e);
                            break;
                        }
                        log_server!("Envoi du paquet MultiplePlayerTransformation réussi");
                    }
                    Err(e) => {
                        log_err_server!("Échec de la génération/envoi du packet. Erreur : {}", e);
                    }
                }
            }
        }
    }

    GAME_STATE.remove_player(&player_id);
    log_server!("Joueur {}: déconnexion.", player_id);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    log_server!("Serveur: lancement.");
    GAME_STATE.init_random_seed();

    const IP: &'static str = "127.0.0.1:5000";
    let listener = tokio::net::TcpListener::bind(IP).await?;
    log_server!("Serveur: démarre à l'adresse {}.", IP);

    loop {
        let (stream, addr) = listener.accept().await?;
        log_server!("Serveur: connexion de l'adresse {}.", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                log_err_server!("Échec du traitement du client.\nErreur : {}", e);
            }
        });
    }
}
