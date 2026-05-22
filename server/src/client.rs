use std::sync::Arc;

use crate::game::{HandlerContext, PacketHandler};
use crate::network::ServerConnection;
use crate::state::AppState;
use anyhow::Result;
use satiscore::log_err_server;
use satiscore::log_server;
use satiscore::network::messages::{self, new_server_seed_paquet, ContenuPaquet, Paquet, PlayerTransformation, TypePaquet};
use satiscore::network::traits::PacketCodec;
use tokio::io::split;
use tokio::net::TcpStream;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Sender as TokioBroadcastSender;
use tokio::sync::mpsc;

/// Taille du canal mpsc entre la boucle de lecture et la tâche d'écriture.
const MAX_PACKETS_IN_MPSC_CHANNEL: usize = 32;

/// Gère le cycle de vie complet d'un client connecté.
/// 1. Handshake : envoi du server_id, réception du paquet de connexion, réponse ack + seed + sync.
/// 2. Boucle read/write : lecture des paquets entrant → handler → envoi des réponses.
/// 3. Nettoyage : retrait du joueur de l'état, arrêt de la tâche d'écriture.
pub struct ClientSession {
    /// Identifiant unique attribué par le serveur.
    player_id: u64,
    /// Nom d'utilisateur extrait du paquet DonneesConnexion.
    username: String,
    /// Connexion chiffrée avec le client.
    conn: ServerConnection,
    /// Gestionnaire de paquets (injecté, testable).
    handler: Box<dyn PacketHandler>,
    /// État partagé du serveur (joueurs, monde).
    state: Arc<AppState>,
    /// Canal broadcast pour diffuser les positions aux autres clients.
    broadcaster: TokioBroadcastSender<Paquet>,
}

impl ClientSession {
    pub fn new(
        player_id: u64,
        server_id: [u8; 16],
        handler: Box<dyn PacketHandler>,
        state: Arc<AppState>,
        broadcaster: TokioBroadcastSender<Paquet>,
    ) -> Self {
        let conn = ServerConnection::new(player_id, server_id);
        Self {
            player_id,
            username: String::new(),
            conn,
            handler,
            state,
            broadcaster,
        }
    }

    /// Point d'entrée : possède la session et le stream, les consume.
    /// Séquences :
    /// - **Handshake** : échange initial non chiffré (server_id), puis paquet chiffré DonneesConnexion.
    /// - **Initialisation** : ajout du joueur, envoi de l'ack, de la seed, et synchronisation
    ///   des positions des joueurs déjà connectés.
    /// - **Boucle read/write** : le stream est split. Une tâche dédiée écrit les réponses
    ///   (mpsc + broadcast). La boucle principale lit les paquets entrants et les dispatch.
    /// - **Nettoyage** : retrait du joueur du registre, arrêt de la tâche d'écriture.
    pub async fn run(mut self, mut stream: TcpStream) -> Result<()> {
        let player_id = self.player_id;

        // Handshake : envoi du server_id (16 octets bruts, non chiffrés)
        self.conn.send_server_id(&mut stream).await?;
        let packet = match self.conn.receive_packet(&mut stream).await {
            Ok(packet_payload) => packet_payload,
            Err(e) => {
                log_err_server!("Échec de la réception du paquet.\nErreur : {}", e);
                return Ok(());
            }
        };

        // Extraction du nom d'utilisateur
        self.username = match &packet.contenu {
            ContenuPaquet::DonneesConnexion { username, .. } => username.clone(),
            _ => format!("Player{}", player_id),
        };

        // Ajout du joueur à l'état partagé
        self.state.add_player(player_id, self.username.clone());
        log_server!("Joueur {} ({}): ajout à l'état global du serveur", self.username, player_id);

        // Le handler traite le paquet de connexion (log)
        let ctx = HandlerContext {
            player_id,
            state: &*self.state,
            broadcaster: &self.broadcaster,
        };
        self.handler.handle(packet, &ctx);

        // Réponse : Fermeture du handshake
        let ack = messages::create_handshake_ack(player_id, 0);
        if let Err(e) = self.conn.send_packet(&mut stream, &ack).await {
            log_err_server!("Échec de l'envoi du handshake ack.\nErreur : {}", e);
            return Ok(());
        }

        // Réponse : seed du monde
        let seed_packet = new_server_seed_paquet(self.state.get_seed());
        if let Err(e) = self.conn.send_packet(&mut stream, &seed_packet).await {
            log_err_server!("Échec de l'envoi de la seed.\nErreur : {}", e);
            return Ok(());
        }
        log_server!("Seed envoyée au joueur {} !", player_id);

        // Synchronisation INITIALE UNIQUEMENT : envoyer les positions des autres joueurs
        if let Some(players) = self.state.get_all_players_vec() {
            let players_data: Vec<PlayerTransformation> = players
                .iter()
                .map(|p| PlayerTransformation {
                    player_id: p.id,
                    position: p.position.clone(),
                    rotation: p.rotation.clone(),
                })
                .collect();

            if !players_data.is_empty() {
                let sync_packet = Paquet::new(
                    TypePaquet::MultiplePlayerTransformation,
                    ContenuPaquet::MultiplePlayerTransformation { data: players_data },
                );
                if let Err(e) = self.conn.send_packet(&mut stream, &sync_packet).await {
                    log_err_server!("Échec de l'envoi du sync initial.\nErreur : {}", e);
                }
            }
        }

        // Séparation lecture/écriture pour la suite
        let (mut read_half, write_half) = split(stream);
        let codec = self.conn.get_codec();

        // Canal mpsc : le read loop envoie les réponses à la tâche d'écriture
        let (write_tx, mut write_rx) = mpsc::channel::<Paquet>(MAX_PACKETS_IN_MPSC_CHANNEL);

        // Tâche d'écriture dédiée
        // Écrit les paquets provenant soit du canal mpsc (réponses directes),
        // soit du canal broadcast (mises à jour de positions des autres joueurs).
        let mut broadcast_rx = self.broadcaster.subscribe();
        let write_task = tokio::spawn(async move {
            let mut write_half = write_half;

            loop {
                tokio::select! {
                    Some(packet) = write_rx.recv() => {
                        if let Err(e) = codec.send_packet(&mut write_half, &packet).await {
                            log_err_server!("Erreur envoi paquet: {}", e);
                            break;
                        }
                    }
                    result = broadcast_rx.recv() => {
                        match result {
                            Ok(packet) => {
                                if let Err(e) = codec.send_packet(&mut write_half, &packet).await {
                                    log_err_server!("Erreur envoi broadcast: {}", e);
                                    break;
                                }
                            }
                            Err(RecvError::Lagged(n)) => {
                                log_server!("{} messages de broadcast perdus pour le joueur {}", n, player_id);
                            }
                            Err(RecvError::Closed) => break,
                        }
                    }
                }
            }
        });

        // Boucle principale de lecture
        // Chaque paquet reçu est passé au handler. Si le handler retourne une
        // réponse, elle est envoyée via le canal mpsc. S'il retourne None,
        // le client est éjecté (paquet non géré ou déconnexion volontaire).
        loop {
            match self.conn.receive_packet(&mut read_half).await {
                Ok(packet) => {
                    let ctx = HandlerContext {
                        player_id,
                        state: self.state.as_ref(),
                        broadcaster: &self.broadcaster,
                    };
                    if let Some(response) = self.handler.handle(packet, &ctx) {
                        if write_tx.send(response).await.is_err() {
                            break;
                        }
                    } else {
                        log_server!("Joueur {}: éjection.", player_id);
                        break;
                    }
                }
                Err(e) => {
                    log_err_server!("Échec de la réception du paquet.\nErreur : {}", e);
                    break;
                }
            }
        }

        // Nettoyage
        drop(write_tx);
        write_task.abort();
        self.state.remove_player(&player_id);
        log_server!("Joueur {}: déconnexion.", player_id);
        Ok(())
    }
}
