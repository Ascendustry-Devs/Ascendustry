//! Types d'erreurs pour la communication réseau.
//!
//! Ce module définit l'enum `NetworkError` qui couvre tous les types d'erreurs
//! possibles lors de l'envoi/réception de paquets sur le réseau.

use std::fmt;

/// Énumération des erreurs réseau possibles.
///
/// Chaque variante représente un type d'erreur différent avec des informations
/// supplémentaires pour le débogage.
///
/// # Variantes
///
/// - `Io` : Erreur d'entrée/sortie (connexion fermée, erreur socket)
/// - `Codec` : Erreur d'encodage/décodage
/// - `PacketTooLarge` : Paquet dépasse la taille maximale autorisée
/// - `InvalidPacket` : Paquet invalide (désérialisation échouée)
/// - `Disconnected` : Connexion fermée par l'hôte distant
/// - `NotConnected` : Pas connecté au serveur
/// - `InvalidData` : Données reçues invalides (trop courtes, format incorrect)
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// Erreur d'entrée/sortie (IO). Contient le message d'erreur original.
    Io(String),
    /// Erreur d'encodage ou de décodage. Contient le message d'erreur.
    Codec(String),
    /// Paquet dépasse la taille maximale. Contient la taille réelle.
    PacketTooLarge(usize),
    /// Paquet invalide (échec de désérialisation). Contient le message d'erreur.
    InvalidPacket(String),
    /// La connexion a été fermée par l'hôte distant.
    Disconnected,
    /// Pas connecté au serveur (appel effectués avant connection).
    NotConnected,
    /// Données invalides reçues. Contient des détails sur l'erreur.
    InvalidData(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Io(e) => write!(f, "IO error: {}", e),
            NetworkError::Codec(e) => write!(f, "Codec error: {}", e),
            NetworkError::PacketTooLarge(size) => write!(f, "Packet too large: {} bytes", size),
            NetworkError::InvalidPacket(e) => write!(f, "Invalid packet: {}", e),
            NetworkError::Disconnected => write!(f, "Connection closed"),
            NetworkError::NotConnected => write!(f, "Not connected"),
            NetworkError::InvalidData(e) => write!(f, "Invalid data: {}", e),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Convertit une erreur IO standard en NetworkError.
impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> Self {
        NetworkError::Io(err.to_string())
    }
}

/// Convertit une String en NetworkError (erreur de codec).
impl From<String> for NetworkError {
    fn from(err: String) -> Self {
        NetworkError::Codec(err)
    }
}

/// Convertit un &str en NetworkError (erreur de codec).
impl From<&str> for NetworkError {
    fn from(err: &str) -> Self {
        NetworkError::Codec(err.to_string())
    }
}
