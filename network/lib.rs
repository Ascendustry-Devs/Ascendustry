pub mod crypto;
pub mod error;
pub mod messages;
pub mod network_protocol;
pub mod traits;

/// Adresse réseau par défaut du serveur (utilisée par client et serveur)
pub const DEFAULT_SERVER_ADDRESS: &str = "127.0.0.1:42677";
