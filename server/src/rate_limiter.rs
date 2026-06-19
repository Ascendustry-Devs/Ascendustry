use std::time::{Duration, Instant};

/// Compteur à fenêtre glissante pour limiter le débit de paquets.
///
/// Fonctionnement : une fenêtre temporelle est définie (ex: 1 seconde).
/// Tant que le nombre de paquets dans cette fenêtre ne dépasse pas le maximum
/// autorisé, les paquets sont acceptés. La fenêtre se réinitialise
/// automatiquement après `window_duration`.
pub struct RateLimiter {
    window_duration: Duration,
    max_packets: usize,
    window_start: Instant,
    packet_count: usize,
}

impl RateLimiter {
    pub fn new(max_packets_per_second: usize) -> Self {
        Self {
            window_duration: Duration::from_secs(1),
            max_packets: max_packets_per_second,
            window_start: Instant::now(),
            packet_count: 0,
        }
    }

    /// Vérifie si un paquet est autorisé et incrémente le compteur.
    /// Retourne `true` si le paquet est dans la limite, `false` sinon.
    pub fn check_and_update(&mut self) -> bool {
        let now = Instant::now();

        if now.duration_since(self.window_start) >= self.window_duration {
            self.window_start = now;
            self.packet_count = 0;
        }

        if self.packet_count >= self.max_packets {
            return false;
        }

        self.packet_count += 1;
        true
    }
}
