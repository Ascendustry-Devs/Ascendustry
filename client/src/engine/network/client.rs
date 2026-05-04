//! Connexion réseau côté client.
//!
//! Ce module gère la connexion TCP côté client. Il encapsulate :
//! - La connexion TCP au serveur
//! - Le runtime Tokio pour les opérations async
//! - Le codec chiffré pour la communication
//! - Une file d'attente pour séquentialiser les envois
//! - Une file d'attente pour recevoir les paquets (non-bloquant)
//!
//! ## Architecture de connexion
//!
//! ```text
//! ClientConnection
//! ├── runtime: Tokio runtime (pour les tâches async)
//! ├── stream: Arc<Mutex<TcpStream>> (connexion TCP)
//! ├── codec: EncryptedCodec (chiffrement)
//! ├── sender: mpsc::UnboundedSender<Paquet> (file d'attente d'envoi)
//! ├── receiver: mpsc::UnboundedReceiver<Paquet> (file d'attente de réception)
//! └── player_id: ID attribué par le serveur
//! ```
//!
//! ## File d'attente d'envoi (Résolution du problème de perte de paquets)
//!
//! Le problème initial : plusieurs tâches concurrentes essayaient d'écrire
//! sur le même TcpStream, causant des pertes de données.
//!
//! Solution : Utiliser un channel pour séquentialiser les envois.
//! - `send_packet()` met le paquet dans le channel (non-bloquant)
//! - Une seule tâche (`start_sender_task`) lit les paquets et les envoie
//! - Cela garantit l'ordre FIFO et élimine les écritures concurrentes
//!
//! ## File d'attente de réception (Non-bloquante)
//!
//! Pour éviter que la réception ne bloque le thread principal :
//! - Une tâche d'arrière-plan reçoit continuellement les paquets
//! - Les paquets reçus sont placés dans un channel
//! - `receive_packet()` récupère les paquets depuis ce channel (non-bloquant)

use shared::network::crypto::compute_shared_secret;
use shared::network::messages::{ContenuPaquet, Paquet, CURRENT_VERSION};
use shared::network::network_protocol::{create_codec, EncryptedCodec};
use shared::network::traits::PacketCodec;
use shared::{log_client, log_err_client};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Mutex};

/// Connexion au serveur.
///
/// Cette structure gère toute la communication réseau avec le serveur :
/// - Connexion TCP
/// - Handshake initial
/// - Envoi de paquets (via file d'attente)
/// - Réception de paquets (via file d'attente, non-bloquante)
///
/// # Utilisation
///
/// ```ignore
/// let mut conn = ClientConnection::new()?;
/// conn.connect("127.0.0.1:5000")?;
/// let (player_id, seed) = conn.perform_handshake("MonJoueur")?;
/// conn.send_packet(packet)?;
/// if let Some(packet) = conn.receive_packet()? {
///     // Traiter le paquet
/// }
/// ```
pub struct ClientConnection {
    /// Runtime Tokio pour exécuter les opérations async
    runtime: Option<Runtime>,
    /// Stream TCP protégé par un Mutex pour l'accès thread-safe
    stream: Option<Arc<Mutex<TcpStream>>>,
    /// Codec chiffré pour la communication
    codec: Arc<EncryptedCodec>,
    /// ID du joueur attribué par le serveur
    player_id: Option<u64>,
    /// Indique si la connexion est établie
    connected: bool,
    /// Channel pour l'envoi séquentialisé des paquets
    sender: Option<mpsc::UnboundedSender<Paquet>>,
    /// Channel pour la réception des paquets (non-bloquante)
    receiver: Option<mpsc::UnboundedReceiver<Paquet>>,
    /// Dernier instant de communication avec le serveur (pour ping)
    last_communication: Instant,
}

impl ClientConnection {
    /// Crée une nouvelle connexion (non encore connectée).
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            runtime: None,
            stream: None,
            codec: Arc::new(create_codec([0u8; 32])),
            player_id: None,
            connected: false,
            sender: None,
            receiver: None,
            last_communication: Instant::now(),
        })
    }

    /// Retourne true si connecté au serveur.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Retourne l'ID du joueur attribué par le serveur.
    pub fn player_id(&self) -> Option<u64> {
        self.player_id
    }

    /// Retourne l'instant du dernier échange.
    pub fn get_last_communication(&self) -> Instant {
        self.last_communication
    }

    /// Connecte au serveur TCP.
    ///
    /// # Arguments
    ///
    /// * `server_addr` - Adresse du serveur (ex: "127.0.0.1:5000")
    ///
    /// # Returns
    ///
    /// * `Ok(())` si la connexion a réussi
    /// * `Err(String)` si la connexion a échoué
    pub fn connect(&mut self, server_addr: &str) -> Result<(), String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;

        let stream = runtime.block_on(TcpStream::connect(server_addr)).map_err(|e| e.to_string())?;

        self.runtime = Some(runtime);
        self.stream = Some(Arc::new(Mutex::new(stream)));
        Ok(())
    }

    /// Effectue le handshake avec le serveur.
    pub fn perform_handshake(&mut self, username: &str) -> Result<(u64, u32), String> {
        let runtime = self.runtime.as_ref().ok_or("No runtime")?;

        // Effectuer le handshake de manière bloquante
        let (player_id, server_seed, codec) = runtime.block_on(async {
            let mut stream = self.stream.as_mut().ok_or("Not connected")?.lock().await;

            let mut server_id_buf = [0u8; 16];
            stream.read_exact(&mut server_id_buf).await.map_err(|e| e.to_string())?;

            let codec = Arc::new(create_codec(compute_shared_secret(&server_id_buf, b"server")));

            let packet = Paquet::new(
                shared::network::messages::TypePaquet::Handshake,
                ContenuPaquet::DonneesConnexion {
                    version: CURRENT_VERSION,
                    username: username.to_string(),
                },
            );

            codec.send_packet(&mut *stream, &packet).await.map_err(|e| e.to_string())?;

            let confirmation_packet = codec.receive_packet(&mut *stream).await.map_err(|e| e.to_string())?;
            let player_id = match confirmation_packet.contenu {
                ContenuPaquet::Confirmation { player_id, .. } => player_id,
                _ => return Err("Échec de la réception du paquet de confirmation.".to_string()),
            };

            let seed_packet = codec.receive_packet(&mut *stream).await.map_err(|e| e.to_string())?;
            let server_seed = match seed_packet.contenu {
                ContenuPaquet::ServerSeed { seed } => seed,
                _ => return Err("Échec de la réception de la seed du serveur.".to_string()),
            };
            log_client!("Seed du serveur reçue : {}.", server_seed);

            Ok((player_id, server_seed as u32, codec))
        })?;

        self.codec = codec;
        self.player_id = Some(player_id);
        self.connected = true;

        self.start_sender_task();
        self.start_receiver_task();

        Ok((player_id, server_seed))
    }

    fn start_sender_task(&mut self) {
        let stream = self.stream.clone();
        let codec = self.codec.clone();

        let (tx, mut rx) = mpsc::unbounded_channel();
        self.sender = Some(tx);

        if let Some(runtime) = self.runtime.as_ref() {
            runtime.spawn(async move {
                while let Some(packet) = rx.recv().await {
                    let mut guard = stream.as_ref().unwrap().lock().await;
                    if let Err(e) = codec.send_packet(&mut *guard, &packet).await {
                        log_err_client!("Échec de l'envoi du packet.\nErreur : {}", e);
                    }
                }
            });
        }
    }

    fn start_receiver_task(&mut self) {
        let stream = self.stream.clone();
        let codec = self.codec.clone();

        let (tx, rx) = mpsc::unbounded_channel();
        self.receiver = Some(rx);

        if let Some(runtime) = self.runtime.as_ref() {
            runtime.spawn(async move {
                loop {
                    let mut guard = stream.as_ref().unwrap().lock().await;
                    match codec.receive_packet(&mut *guard).await {
                        Ok(packet) => {
                            match packet.contenu.clone() {
                                ContenuPaquet::MultiplePlayerTransformation { data } => {
                                    // todo!("Ajout/Mise à jour des informations au game state")
                                    // for i in data {
                                    //     log_client!("Données du joueur {} reçues: {:?}", i.player_id, i.position);
                                    // }
                                }
                                _ => {}
                            }
                            drop(guard);
                            if tx.send(packet).is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            });
        }
    }

    /// Envoie un paquet au serveur (non-bloquant).
    pub fn send_packet(&mut self, packet: Paquet) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        if let Some(sender) = &self.sender {
            sender.send(packet).map_err(|e| e.to_string())?;
        }

        self.last_communication = Instant::now();

        Ok(())
    }

    /// Reçoit un paquet du serveur (méthode non-bloquante).
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Paquet))` si un paquet a été reçu
    /// * `Ok(None)` si aucun paquet n'est disponible
    /// * `Err(String)` en cas d'erreur
    pub fn receive_packet(&mut self) -> Result<Option<Paquet>, String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        if let Some(receiver) = &mut self.receiver {
            match receiver.try_recv() {
                Ok(packet) => {
                    self.last_communication = Instant::now();
                    Ok(Some(packet))
                }
                Err(mpsc::error::TryRecvError::Empty) => Ok(None),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.connected = false;
                    Err("Connexion fermée par le serveur".to_string())
                }
            }
        } else {
            Err("Receiver not initialized".to_string())
        }
    }
}
