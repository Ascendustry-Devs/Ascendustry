use crate::game::validator::ChunkValidator;
use crate::state::GAME_STATE;
use shared::network::messages::*;
use shared::network::messages::{self, ContenuPaquet, Paquet};
use shared::*;

pub struct PacketHandler {
    validator: ChunkValidator,
}

impl PacketHandler {
    pub fn new() -> Self {
        Self {
            validator: ChunkValidator::new(),
        }
    }

    pub fn handle_packet(&mut self, packet: Paquet) -> Option<Paquet> {
        match &packet.contenu {
            ContenuPaquet::DonneesConnexion { version, username } => {
                log_server!("Joueur {} se connecte avec la version {}", username, version);
                Some(packet)
            }

            ContenuPaquet::PlayerTransformation { data } => {
                GAME_STATE.update_player_position(data.player_id, data.position.clone(), data.rotation.clone());
                Some(packet)
            }

            ContenuPaquet::ChunkValidationBatchRequest { chunks } => {
                log_server!("Reception ChunkValidationBatchRequest avec {} chunks", chunks.len());

                // Répondre immédiatement sans bloquer le thread async
                // La validation lourde sera faite en arrière-plan plus tard
                let chunks_vec = chunks.clone();
                tokio::spawn(async move {
                    let seed = GAME_STATE.get_seed();
                    let mut validator = ChunkValidator::new();
                    let _results = validator.validate_batch(chunks_vec, seed, &GAME_STATE);
                });

                Some(messages::new_chunk_validation_batch_response(vec![]))
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
