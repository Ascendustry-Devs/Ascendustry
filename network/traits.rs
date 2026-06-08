//! Définition des traits réseau partagés.
//!
//! Ce module définit le trait `PacketCodec` qui abstraction l'envoi et la réception
//! de paquets sur n'importe quel flux asyncrhone compatible avec Tokio.

use std::future::Future;

use crate::error::NetworkError;
use crate::messages::Paquet;
use tokio::io::{AsyncRead, AsyncWrite};

/// Trait pour l'envoi et la réception de paquets chiffrés.
///
/// Ce trait définit l'interface de base pour la communication réseau.
/// Il est implémenté par `EncryptedCodec` qui ajoute le chiffrement XOR.
///
/// # Implémentation required
///
/// Les implémenteurs doivent :
/// - Implémenter `Clone + Send + Sync` (nécessaire pour le partage entre tâches)
/// - Gérer l'encodage/décodage des paquets
/// - Gérer le chiffrement si nécessaire
///
/// # Exemple d'utilisation
///
/// ```ignore
/// // Envoi d'un paquet
/// codec.send_packet(&mut stream, &packet).await?;
///
/// // Réception d'un paquet
/// let packet = codec.receive_packet(&mut stream).await?;
/// ```
pub trait PacketCodec: Clone + Send + Sync {
    /// Envoie un paquet sur le flux fourni.
    ///
    /// # Arguments
    ///
    /// * `stream` - Flux asyncrhone (typiquement un `TcpStream`)
    /// * `packet` - Référence vers le paquet à envoyer
    ///
    /// # Returns
    ///
    /// * `Ok(())` si l'envoi a réussi
    /// * `Err(NetworkError)` en cas d'erreur
    fn send_packet<S: AsyncWrite + Unpin>(
        &self,
        stream: &mut S,
        packet: &Paquet,
    ) -> impl Future<Output = Result<(), NetworkError>>;

    /// Reçoit un paquet du flux fourni.
    ///
    /// # Arguments
    ///
    /// * `stream` - Flux asyncrhone (typiquement un `TcpStream`)
    ///
    /// # Returns
    ///
    /// * `Ok(Paquet)` si la réception a réussi
    /// * `Err(NetworkError)` en cas d'erreur (connexion fermée, données invalides, etc.)
    fn receive_packet<S: AsyncRead + Unpin>(&self, stream: &mut S) -> impl Future<Output = Result<Paquet, NetworkError>>;
}
