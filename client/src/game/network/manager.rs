//! Gestionnaire réseau de jeu côté client.
//!
//! Ce module orchestre la communication réseau pour la logique de jeu.
//! Il fait la ponte entre :
//! - La couche réseau bas-niveau (`ClientConnection`)
//! - Le protocole applicatif (`GameProtocol`)
//! - La logique de rate limiting (limitation des envois)
//!
//! ## Architecture
//!
//! ```text
//! NetworkManager
//! ├── connection: ClientConnection (connexion TCP + envoi/réception)
//! ├── protocol: Option<GameProtocol> (création des paquets)
//! ├── last_send: Instant (pour le rate limiting des positions)
//! └── server_seed: Option<u64> (seed du serveur pour la génération)
//! ```
//!
//! ## Rate Limiting
//!
//! Le client limite l'envoi des positions à un intervalle fixe (50ms).
//! Cela évite de saturer le réseau avec des mises à jour trop fréquentes.

use crate::engine::network::ClientConnection;
use crate::game::network::protocol::GameProtocol;
use shared::{log_client, log_err_client, network::messages::Paquet};
use std::time::{Duration, Instant};

/// Intervalle entre deux envois de position (50ms = 20 updates/sec)
const POSITION_UPDATE_INTERVAL: Duration = Duration::from_millis(50);

/// Gestionnaire réseau pour le jeu.
///
/// Cette structure est utilisée par le moteur de jeu pour gérer toutes
/// les communications avec le serveur. Elle fournit une API de haut niveau
/// pour :
/// - Se connecter au serveur
/// - Envoyer les positions du joueur
/// - Envoyer les validations de chunks
/// - Recevoir les paquets du serveur
pub struct NetworkManager {
    /// Connexion réseau bas-niveau
    connection: ClientConnection,
    /// Protocol de création des paquets (optionnel avant handshake)
    protocol: Option<GameProtocol>,
    /// Dernier instant d'envoi de position (pour rate limiting)
    last_send: Instant,
    /// Seed du serveur (pour la génération de chunks)
    server_seed: Option<u64>,
}

impl NetworkManager {
    /// Crée un nouveau gestionnaire réseau.
    ///
    /// Initialise la connexion mais ne se connecte pas au serveur.
    pub fn new() -> Self {
        Self {
            connection: ClientConnection::new().expect("Failed to create client connection"),
            protocol: None,
            last_send: Instant::now(),
            server_seed: None,
        }
    }

    /// Retourne true si connecté au serveur.
    pub fn is_connected(&self) -> bool {
        self.connection.is_connected()
    }

    /// Retourne l'ID du joueur.
    pub fn player_id(&self) -> Option<u64> {
        self.connection.player_id()
    }

    /// Retourne l'instant du dernier échange.
    pub fn get_last_communication(&self) -> Instant {
        self.connection.get_last_communication()
    }

    /// Se connecte au serveur.
    ///
    /// Établit la connexion TCP mais ne fait pas encore le handshake.
    ///
    /// # Arguments
    ///
    /// * `server_addr` - Adresse du serveur (ex: "127.0.0.1:5000")
    pub fn connect(&mut self, server_addr: &str) {
        log_client!("NetworkManager: tentative de connexion...");
        if let Err(e) = self.connection.connect(server_addr) {
            log_err_client!("NetworkManager: échec de la tentative de connexion au serveur.\nErreur : {}", e);
        }
    }

    /// Effectue le handshake avec le serveur.
    ///
    /// Échange les informations initiales avec le serveur :
    /// - Envoi du nom d'utilisateur
    /// - Réception de l'ID de joueur
    /// - Réception de la seed du serveur
    ///
    /// # Arguments
    ///
    /// * `username` - Nom d'utilisateur du joueur
    ///
    /// # Returns
    ///
    /// * `Ok(player_id)` si le handshake a réussi
    /// * `Err(String)` sinon
    pub fn perform_handshake(&mut self, username: &str) -> Result<u64, String> {
        log_client!("NetworkManager: handshake...");
        match self.connection.perform_handshake(username) {
            Ok((id, seed)) => {
                // Créer le protocole de jeu avec l'ID du joueur
                self.protocol = Some(GameProtocol::new(id));
                self.server_seed = Some(seed as u64);
                log_client!("NetworkManager: connexion établie !");
                Ok(id)
            }
            Err(e) => {
                log_err_client!("NetworkManager: échec du handshake.\nErreur : {}", e);
                Err(e)
            }
        }
    }

    /// Retourne la seed du serveur (pour la génération de chunks).
    pub fn get_server_seed(&self) -> Option<u32> {
        self.server_seed.map(|s| s as u32)
    }

    /// Envoie la position du joueur au serveur.
    ///
    /// Cette méthode applique un rate limiting :
    /// elle n'envoie la position que si plus de 50ms se sont écoulées
    /// depuis le dernier envoi. Cela évite de saturer le réseau.
    ///
    /// # Arguments
    ///
    /// * `x, y, z` - Position du joueur
    /// * `rx, ry` - Rotation du joueur
    pub fn send_position(&mut self, x: f32, y: f32, z: f32, rx: f32, ry: f32) -> Result<(), String> {
        let now = Instant::now();

        // Vérifier si assez de temps s'est écoulé
        if now.duration_since(self.last_send) >= POSITION_UPDATE_INTERVAL {
            if let Some(protocol) = &self.protocol {
                // Créer le paquet de position
                let packet = protocol.create_position_update(x, y, z, rx, ry);

                // Envoyer via la connexion
                self.connection.send_packet(packet)?;

                // Mettre à jour le timestamp
                self.last_send = now;
            }
        }

        Ok(())
    }

    pub fn send_ping(&mut self, timestamp: u64) -> Result<(), String> {
        if let Some(protocol) = &self.protocol {
            let packet = protocol.create_ping(timestamp);
            self.connection.send_packet(packet)
        } else {
            Ok(())
        }
    }

    pub fn send_pong(&mut self, timestamp: u64) -> Result<(), String> {
        if let Some(protocol) = &self.protocol {
            let packet = protocol.create_pong(timestamp);
            self.connection.send_packet(packet)
        } else {
            Ok(())
        }
    }

    /// Reçoit un paquet du serveur (non-bloquant).
    ///
    /// Cette méthode retourne immédiatement :
    /// - `Ok(Some(Paquet))` si un paquet a été reçu
    /// - `Ok(None)` si aucun paquet n'est disponible
    /// - `Err(String)` en cas d'erreur
    pub fn receive_packet(&mut self) -> Result<Option<Paquet>, String> {
        self.connection.receive_packet()
    }
}
