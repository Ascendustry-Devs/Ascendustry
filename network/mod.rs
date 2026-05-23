//! Module réseau partagé entre le client et le serveur.
//!
//! Ce module fournit l'infrastructure de base pour la communication réseau :
//! - `crypto` : Fonctions cryptographiques (XOR cipher, SHA-256)
//! - `messages` : Définition des types de paquets (Paquet, ContenuPaquet, TypePaquet)
//! - `network_protocol` : Codec de chiffrement pour l'encodage/décodage des paquets
//! - `traits` : Définition du trait PacketCodec pour l'envoi/réception de paquets
//! - `error` : Types d'erreurs réseau
//!
//! ## Architecture réseau
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        APPLICATION                              │
//! │  ┌─────────────────────┐       ┌─────────────────────┐        │
//! │  │  Game Protocol      │       │  Packet Handler     │        │
//! │  │  (création packets) │       │  (traitement msgs)   │        │
//! │  └──────────┬──────────┘       └──────────┬──────────┘        │
//! │             │                             │                   │
//! │             ▼                             ▼                   │
//! │  ┌─────────────────────────────────────────────────────────┐  │
//! │  │              PacketCodec (trait)                         │  │
//! │  │   - send_packet(&self, stream, &packet)                  │  │
//! │  │   - receive_packet(&self, stream) -> Paquet              │  │
//! │  └──────────────────────────┬──────────────────────────────┘  │
//! │                             │                                   │
//! │                             ▼                                   │
//! │  ┌─────────────────────────────────────────────────────────┐  │
//! │  │              EncryptedCodec                              │  │
//! │  │   - Chiffrement XOR avec clé partagée                   │  │
//! │  │   - Encodage/décodage avec préfixe de longueur           │  │
//! │  └──────────────────────────┬──────────────────────────────┘  │
//! │                             │                                   │
//! └─────────────────────────────┼───────────────────────────────────┘
//!                               │
//!                               ▼
//!                    ┌────────────────────┐
//!                    │   TcpStream        │
//!                    │   (tokio)          │
//!                    └────────────────────┘
//! ```
//!
//! ## Flux d'un paquet (client → serveur)
//!
//! 1. **Création** : `GameProtocol::create_chunk_validation_request()`
//! 2. **Encodage** : `EncryptedCodec::encode(&packet)` → JSON chiffré
//! 3. **Envoi longueur + données** : `send_packet()` ajoute le préfixe 4 octets
//! 4. **Réception** : `receive_packet()` lit la longueur, puis les données
//! 5. **Décodage** : `EncryptedCodec::decode()` → déchiffrement, désérialisation
//! 6. **Traitement** : `PacketHandler::handle_packet()`

pub mod crypto;
pub mod error;
pub mod messages;
pub mod network_protocol;
pub mod traits;

pub use error::NetworkError;
pub use network_protocol::{
    create_cipher, create_codec, create_server_id, perform_client_handshake, perform_server_handshake, EncryptedCodec,
};
pub use traits::PacketCodec;
