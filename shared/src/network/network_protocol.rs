//! Protocole réseau avec chiffrement XOR.
//!
//! Ce module implémente le protocole de communication utilisé par le client et le serveur.
//! Il fournit :
//! - `Cipher` : Chiffrement XOR avec clé de 32 octets
//! - `EncryptedCodec` : Codec complet pour l'envoi/réception de paquets chiffrés
//!
//! ## Format d'un paquet
//!
//! ```text
//! [4 octets: longueur][N octets: données chiffrées]
//! ```
//!
//! ## Protocole de handshake
//!
//! 1. Le serveur génère un `server_id` aléatoire de 16 octets et l'envoie non chiffré
//! 2. Le client calcule la clé partagée avec `compute_shared_secret(server_id, "server")`
//! 3. Le client envoie un paquet `Handshake` chiffré
//! 4. Le serveur répond avec `HandshakeAck` et `ServerSeed` chiffrés

use crate::network::crypto::{compute_shared_secret, generate_server_id, server_id_to_hex, xor_crypt};
use crate::network::error::NetworkError;
use crate::network::messages::{Paquet, MAX_PAQUET_SIZE};
use crate::network::traits::PacketCodec;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Chiffrement XOR simple avec une clé de 32 octets.
///
/// Ce cipher utilise un XOR simple avec la clé pour chiffrer/déchiffrer les données.
/// Il n'offre pas de sécurité cryptographique forte, mais suffit pour une protection
/// basique contre l'inspection visuelle des données réseau.
///
/// # Utilisation
///
/// ```ignore
/// let cipher = Cipher::from_shared_secret(shared_secret);
/// let encrypted = cipher.encrypt(&data);
/// let decrypted = cipher.decrypt(&encrypted);
/// ```
#[derive(Debug, Clone)]
pub struct Cipher {
    /// Clé de chiffrement de 32 octets
    key: [u8; 32],
}

impl Cipher {
    /// Crée un nouveau Cipher avec la clé fournie.
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Crée un Cipher à partir d'un secret partagé de 32 octets.
    pub fn from_shared_secret(shared_secret: [u8; 32]) -> Self {
        Self { key: shared_secret }
    }

    /// Chiffre les données fournies en utilisant XOR avec la clé.
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        xor_crypt(data, &self.key)
    }

    /// Déchiffre les données fournies (XOR est sa propre inverse).
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        xor_crypt(data, &self.key)
    }
}

/// Codec de chiffrement pour l'envoi et la réception de paquets.
///
/// Ce codec combine :
/// - Le chiffrement XOR avec `Cipher`
/// - La sérialisation/désérialisation JSON des paquets
/// - L'ajout d'un préfixe de longueur (4 octets) pour la réception
///
/// # Format d'envoi
///
/// 1. Sérialiser le paquet en JSON
/// 2. Chiffrer avec XOR
/// 3. Ajouter la longueur (4 octets big-endian)
/// 4. Envoyer longueur + données
///
/// # Format de réception
///
/// 1. Lire 4 octets pour la longueur
/// 2. Lire les données chiffrées
/// 3. Déchiffrer avec XOR
/// 4. Désérialiser en paquet
#[derive(Clone)]
pub struct EncryptedCodec {
    /// Le cipher utilisé pour le chiffrement
    cipher: Arc<Cipher>,
}

impl EncryptedCodec {
    /// Crée un nouveau EncryptedCodec avec le cipher fourni.
    pub fn new(cipher: Arc<Cipher>) -> Self {
        Self { cipher }
    }

    /// Encode un paquet pour l'envoi.
    ///
    /// Étapes :
    /// 1. Sérialise le paquet en JSON
    /// 2. Chiffre avec XOR
    ///
    /// # Returns
    ///
    /// Un vecteur d'octets prêt à être envoyé sur le réseau
    /// (le préfixe de longueur est ajouté par send_packet())
    pub fn encode(&self, packet: &Paquet) -> Vec<u8> {
        // Étape 1: Sérialisation JSON
        let serialized = packet.serialize();

        // Étape 2: Chiffrement XOR
        let encrypted = self.cipher.encrypt(&serialized);

        encrypted
    }

    /// Décode les données reçues en paquet.
    ///
    /// Étapes :
    /// 1. Déchiffrer les données
    /// 2. Désérialiser en paquet
    ///
    /// # Returns
    ///
    /// Le paquet désérialisé
    /// (le préfixe de longueur a déjà été retiré par receive_packet())
    pub fn decode(&self, data: &[u8]) -> Result<Paquet, NetworkError> {
        // Déchiffrement
        let decrypted = self.cipher.decrypt(data);

        // Désérialisation
        Paquet::deserialize(&decrypted).map_err(|e| NetworkError::InvalidPacket(e.to_string()))
    }

    /// Retourne une référence vers le cipher utilisé.
    pub fn cipher(&self) -> Arc<Cipher> {
        self.cipher.clone()
    }
}

/// Implémentation du trait PacketCodec pour EncryptedCodec.
///
/// Cette implémentation gère l'envoi et la réception de paquets sur un flux TCP.
impl PacketCodec for EncryptedCodec {
    /// Envoie un paquet sur le flux TCP.
    ///
    /// Écrit d'abord la longueur (4 octets), puis les données chiffrées.
    async fn send_packet<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
        &self,
        stream: &mut S,
        packet: &Paquet,
    ) -> Result<(), NetworkError> {
        // Encode le paquet (sérialisation + chiffrement + préfixe)
        let data = self.encode(packet);
        let len = data.len() as u32;

        // Écriture sur le réseau
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&data).await?;
        stream.flush().await?;

        Ok(())
    }

    /// Reçoit un paquet du flux TCP.
    ///
    /// Lit d'abord la longueur (4 octets), puis les données chiffrées.
    async fn receive_packet<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
        &self,
        stream: &mut S,
    ) -> Result<Paquet, NetworkError> {
        // Lecture de la longueur
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Vérification de la taille maximale
        if len > MAX_PAQUET_SIZE {
            return Err(NetworkError::PacketTooLarge(len));
        }

        // Lecture des données
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;

        // Décodage
        self.decode(&data)
    }
}

/// Effectue le handshake côté client.
///
/// Calcule la clé partagée à partir du server_id et d'un token client.
pub fn perform_client_handshake(server_id_bytes: &[u8], client_token: &[u8]) -> [u8; 32] {
    compute_shared_secret(server_id_bytes, client_token)
}

/// Effectue le handshake côté serveur.
///
/// Calcule la clé partagée à partir du server_id et du token "server".
pub fn perform_server_handshake(server_id_bytes: &[u8]) -> [u8; 32] {
    compute_shared_secret(server_id_bytes, b"server")
}

/// Génère un nouveau server_id aléatoire.
///
/// # Returns
///
/// * `([u8; 16], String)` - Le server_id brut et sa représentation hexadécimale
pub fn create_server_id() -> ([u8; 16], String) {
    let id = generate_server_id();
    let hex = server_id_to_hex(&id);
    (id, hex)
}

/// Crée un Cipher à partir d'un secret partagé.
pub fn create_cipher(shared_secret: [u8; 32]) -> Arc<Cipher> {
    Arc::new(Cipher::from_shared_secret(shared_secret))
}

/// Crée un EncryptedCodec à partir d'un secret partagé.
pub fn create_codec(shared_secret: [u8; 32]) -> EncryptedCodec {
    let cipher = create_cipher(shared_secret);
    EncryptedCodec::new(cipher)
}
