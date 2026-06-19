use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Durée maximum sans recevoir de paquet avant déconnexion.
    pub connection_timeout: Duration,
    /// Nombre maximum de paquets acceptés par seconde par connexion.
    pub max_packets_per_second: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(60),
            max_packets_per_second: 60,
        }
    }
}
