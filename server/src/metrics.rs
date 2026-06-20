use std::sync::atomic::{AtomicU64, Ordering};

/// Collecte des métriques de performance pour le suivi de santé du serveur.
/// Tous les compteurs sont atomiques pour pouvoir être mis à jour depuis
/// n'importe quel thread sans verrou, minimisant la surcharge.
pub struct ServerMetrics {
    /// Durée cumulée des cycles de garde en nanosecondes.
    pub guard_cycle_cumul_ns: AtomicU64,
    /// Nombre de cycles de garde effectués.
    pub guard_cycle_count: AtomicU64,
    /// Nombre total de paquets reçus toutes sessions confondues.
    pub packets_received: AtomicU64,
    /// Nombre total de paquets rejetés par les limiteurs de débit.
    pub packets_rejected: AtomicU64,
    /// Nombre total de connexions acceptées depuis le démarrage.
    pub total_connections: AtomicU64,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            guard_cycle_cumul_ns: AtomicU64::new(0),
            guard_cycle_count: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            packets_rejected: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
        }
    }

    pub fn record_guard_cycle(&self, duration_ns: u64) {
        self.guard_cycle_cumul_ns.fetch_add(duration_ns, Ordering::Relaxed);
        self.guard_cycle_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_packet_received(&self) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_packet_rejected(&self) {
        self.packets_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }
}
