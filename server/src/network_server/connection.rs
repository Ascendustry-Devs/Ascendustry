//! Connexion réseau côté serveur.
//!
//! Ce module gère la connexion TCP côté serveur et encapsulate le codec de chiffrement.
//! Il fournit des méthodes de haut niveau pour l'envoi et la réception de paquets.

use network::crypto::compute_shared_secret;
use network::error::NetworkError;
use network::messages::Paquet;
use network::network_protocol::{create_codec, EncryptedCodec};
use network::traits::PacketCodec;

/// Représente une connexion avec un client.
///
/// Cette structure encapsulate :
/// - Le codec chiffré pour la communication
/// - Le server_id pour le handshake initial
///
/// # Utilisation
///
/// ```ignore
/// let conn = ServerConnection::new(player_id, server_id);
/// conn.send_server_id(&mut stream).await?;
/// let packet = conn.receive_packet(&mut stream).await?;
/// conn.send_packet(&mut stream, &response).await?;
/// ```
pub struct ServerConnection {
    /// Codec chiffré pour l'envoi/réception de paquets
    codec: EncryptedCodec,
    /// ID unique du serveur pour le handshake
    server_id: [u8; 16],
}

impl ServerConnection {
    /// Crée une nouvelle connexion serveur.
    ///
    /// # Arguments
    ///
    /// * `player_id` - ID unique du joueur (utilisé pour le logging)
    /// * `server_id` - ID du serveur généré aléatoirement
    ///
    /// # Détails
    ///
    /// Le codec est initialisé avec la clé partagée calculée à partir
    /// de `compute_shared_secret(server_id, b"server")`.
    pub fn new(player_id: u64, server_id: [u8; 16]) -> Self {
        let codec = create_codec(compute_shared_secret(&server_id, b"server"));
        let _ = player_id; // Réservé pour usage futur (logging, etc.)
        Self { codec, server_id }
    }

    /// Envoie un paquet au client.
    ///
    /// Délègue à `EncryptedCodec::send_packet()`.
    pub async fn send_packet<S: tokio::io::AsyncWrite + Unpin>(&self, stream: &mut S, packet: &Paquet) -> Result<(), NetworkError> {
        self.codec.send_packet(stream, packet).await
    }

    /// Reçoit un paquet du client.
    ///
    /// Délègue à `EncryptedCodec::receive_packet()`.
    pub async fn receive_packet<S: tokio::io::AsyncRead + Unpin>(&self, stream: &mut S) -> Result<Paquet, NetworkError> {
        self.codec.receive_packet(stream).await
    }

    /// Envoie le server_id au client (premier message, non chiffré).
    ///
    /// Cette méthode est appelée lors de l'établissement de la connexion
    /// avant le handshake chiffré. Le server_id permet au client de
    /// calculer la clé partagée.
    ///
    /// # Format
    ///
    /// Envoie directement les 16 octets du server_id sans chiffrement.
    pub async fn send_server_id<S: tokio::io::AsyncWrite + Unpin>(&self, stream: &mut S) -> Result<(), NetworkError> {
        use tokio::io::AsyncWriteExt;
        stream.write_all(&self.server_id).await?;
        stream.flush().await?;
        Ok(())
    }

    /// Retourne une copie du codec pour utilisation dans d'autres tâches.
    pub fn get_codec(&self) -> EncryptedCodec {
        self.codec.clone()
    }
}
