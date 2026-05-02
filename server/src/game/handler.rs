use std::time::SystemTime;

use crate::state::GAME_STATE;
use shared::network::messages::*;
use shared::*;

pub struct PacketHandler {}

impl PacketHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle_packet(&mut self, packet: Paquet) -> Option<Paquet> {
        match &packet.contenu {
            ContenuPaquet::DonneesConnexion { version, username } => {
                log_server!("Joueur {}: connexion avec la version {}.", username, version);
                Some(packet)
            }

            ContenuPaquet::PlayerTransformation { data } => {
                GAME_STATE.update_player_position(data.player_id, data.position.clone(), data.rotation.clone());
                Some(packet)
            }

            ContenuPaquet::Ping { timestamp } => {
                log_server!(
                    "Ping d'il y a {}s reçu.",
                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() - *timestamp
                );
                Some(new_pong_paquet(*timestamp))
            }

            ContenuPaquet::Pong { timestamp } => {
                log_server!(
                    "Pong d'il y a {}s reçu.",
                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() - *timestamp
                );
                Some(packet)
            }

            _ => None,
        }
    }

    pub fn get_players_position_packet(&mut self) -> Result<Paquet, std::io::Error> {
        let players = GAME_STATE.get_all_players_vec();
        match players {
            Some(players_vec) => {
                let players_vec = players_vec
                    .into_iter()
                    .map(|player| PlayerTransformation {
                        player_id: player.id,
                        position: player.position,
                        rotation: player.rotation,
                    })
                    .collect();

                return Ok(Paquet::new(
                    TypePaquet::MultiplePlayerTransformation,
                    ContenuPaquet::MultiplePlayerTransformation { data: players_vec },
                ));
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "La génération du packet a échouée",
                ));
            }
        }
    }
}

impl Default for PacketHandler {
    fn default() -> Self {
        Self::new()
    }
}
