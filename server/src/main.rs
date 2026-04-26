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

static NEXT_PLAYER_ID: AtomicU64 = AtomicU64::new(1);

async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let player_id = NEXT_PLAYER_ID.fetch_add(1, Ordering::SeqCst);

    let server_id = generate_server_id();
    log_server!("Nouveau joueur avec ID: {} (Server ID: {:02x?})", player_id, server_id);

    let conn = ServerConnection::new(player_id, server_id);

    conn.send_server_id(&mut stream).await?;

    let packet = match conn.receive_packet(&mut stream).await {
        Ok(p) => p,
        Err(e) => {
            log_err_server!("Erreur reception: {}", e);
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
    log_server!("Joueur {} ajoute a l'etat du jeu", username);

    // Créer le gestionnaire de paquets pour ce client
    let mut packet_handler = PacketHandler::new();

    packet_handler.handle_packet(packet);

    let ack = messages::create_handshake_ack(player_id, 0);
    if let Err(e) = conn.send_packet(&mut stream, &ack).await {
        log_err_server!("Erreur envoi handshake ack: {}", e);
        return Ok(());
    }

    let seed_packet = new_server_seed_paquet(GAME_STATE.get_seed());
    if let Err(e) = conn.send_packet(&mut stream, &seed_packet).await {
        log_err_server!("Erreur envoi de la seed: {}", e);
        return Ok(());
    } else {
        log_server!("La Seed a ete envoyee au joueur {}", player_id);
    }

    // Loop de check
    loop {
        match conn.receive_packet(&mut stream).await {
            Ok(packet) => {
                if let Some(response) = packet_handler.handle_packet(packet) {
                    conn.send_packet(&mut stream, &response).await?;
                } else {
                    log_server!("Le joueur {} a ete ejecte", player_id);
                    break;
                }
            }
            Err(e) => {
                log_err_server!("Erreur reception paquet: {}", e);
                break;
            }
        }

        // Update pub player's info packet handler
        match packet_handler.get_players_position_packet() {
            Ok(packet) => {
                conn.send_packet(&mut stream, &packet).await.unwrap();
            }
            Err(e) => {
                log_err_server!("Failed to generate packet, {}", e);
            }
        }
    }

    GAME_STATE.remove_player(&player_id);
    log_server!("Joueur {} deconnecte", player_id);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    log_server!("Démarrage de l'initialisation du serveur");
    GAME_STATE.init_random_seed();

    // log_server!("Début de la prégénération du monde");
    // let p1 = Point3::new(0, 0, 1);
    // time!(format!("Temps pris par la prégénération"), {
    //     GAME_STATE.generate_chunks_in_radius(p1, 12);
    // });
    log_server!("Fin de la prégénération du monde");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000").await?;
    log_server!("Serveur demarre sur 127.0.0.1:5000");

    loop {
        let (stream, addr) = listener.accept().await?;
        log_server!("Connexion de {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                log_err_server!("Erreur handling client: {}", e);
            }
        });
    }
}
