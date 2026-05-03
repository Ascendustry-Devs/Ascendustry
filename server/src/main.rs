//! Serveur Satisfactorio - Point d'entrée.

mod game;
mod network;
mod state;

use crate::game::PacketHandler;
use crate::network::ServerConnection;
use crate::state::GAME_STATE;
use anyhow::Result;
use clap::Parser;
use shared::network::crypto::generate_server_id;
use shared::network::messages::{self, new_server_seed_paquet};
use shared::network::traits::PacketCodec;
use shared::*;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::split;
use tokio::net::*;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

static NEXT_PLAYER_ID: AtomicU64 = AtomicU64::new(1);
const SERVER_CONN_WRITE_UPDATE_RATE_PER_SECOND: f64 = 100.0;
const MAX_PACKETS_IN_MPSC_CHANNEL: usize = 32;

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

    // Séparer le stream en lecture et écriture
    let (mut read_half, write_half) = split(stream);
    let codec = conn.get_codec();

    // Channel pour envoyer des paquets à la tâche d'écriture
    let (write_tx, mut write_rx) = mpsc::channel::<shared::network::messages::Paquet>(MAX_PACKETS_IN_MPSC_CHANNEL);

    // Tâche d'écriture dédiée
    let write_task = tokio::spawn(async move {
        let mut write_half = write_half; // Ré-assigner comme mutable
        let mut interval = interval(Duration::from_secs_f64(1.0 / SERVER_CONN_WRITE_UPDATE_RATE_PER_SECOND));

        loop {
            tokio::select! {
                // Recevoir un paquet à envoyer depuis le channel (réponse d'un paquet par exemple)
                Some(packet) = write_rx.recv() => {
                    if let Err(e) = codec.send_packet(&mut write_half, &packet).await {
                        log_err_server!("Erreur envoi paquet: {}", e);
                        break;
                    }
                }
                // Envoi périodique des positions des joueurs
                _ = interval.tick() => {
                    let players = crate::state::GAME_STATE.get_all_players_vec();
                    if let Some(players_vec) = players {
                        let players_data: Vec<_> = players_vec
                            .into_iter()
                            .map(|player| shared::network::messages::PlayerTransformation {
                                player_id: player.id,
                                position: player.position,
                                rotation: player.rotation,
                            })
                            .collect();

                        let packet = shared::network::messages::Paquet::new(
                            shared::network::messages::TypePaquet::MultiplePlayerTransformation,
                            shared::network::messages::ContenuPaquet::MultiplePlayerTransformation { data: players_data },
                        );

                        if let Err(e) = codec.send_packet(&mut write_half, &packet).await {
                            log_err_server!("Erreur envoi positions: {}", e);
                            break;
                        }
                        // log_server!("Envoi du paquet MultiplePlayerTransformation réussi");
                    }
                }
            }
        }
    });

    // Boucle principale de lecture
    loop {
        match conn.receive_packet(&mut read_half).await {
            Ok(packet) => {
                if let Some(response) = packet_handler.handle_packet(packet) {
                    // Envoyer la réponse via le channel
                    if write_tx.send(response).await.is_err() {
                        break;
                    }
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

    // Clean
    drop(write_tx);
    write_task.abort();
    GAME_STATE.remove_player(&player_id);
    log_server!("Joueur {}: déconnexion.", player_id);
    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("127.0.0.1:42677"))]
    address: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    log_server!("Serveur: lancement.");
    GAME_STATE.init_random_seed();

    let args = Args::parse();

    let listener = tokio::net::TcpListener::bind(&args.address).await?;
    log_server!("Serveur: démarre à l'adresse {}.", args.address);

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
