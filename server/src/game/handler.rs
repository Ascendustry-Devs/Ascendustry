use std::time::SystemTime;

use crate::state::AppState;
use network::messages::*;
use satiscore::log_server;
use tokio::sync::broadcast;

pub struct HandlerContext<'a> {
    pub player_id: u64,
    pub state: &'a AppState,
    pub broadcaster: &'a broadcast::Sender<Paquet>,
}

pub trait PacketHandler: Send + Sync {
    fn handle(&self, packet: Paquet, ctx: &HandlerContext) -> Option<Paquet>;
}

pub struct ProductionHandler;

impl PacketHandler for ProductionHandler {
    fn handle(&self, packet: Paquet, ctx: &HandlerContext) -> Option<Paquet> {
        match &packet.contenu {
            ContenuPaquet::DonneesConnexion { version, username } => {
                log_server!("Joueur {}: connexion avec la version {}.", username, version);
                Some(packet)
            }

            ContenuPaquet::PlayerTransformation { data } => {
                ctx.state
                    .update_player_position(data.player_id, data.position.clone(), data.rotation.clone());

                let broadcast_packet = Paquet::new(
                    TypePaquet::MultiplePlayerTransformation,
                    ContenuPaquet::MultiplePlayerTransformation {
                        data: vec![PlayerTransformation {
                            player_id: data.player_id,
                            position: data.position.clone(),
                            rotation: data.rotation.clone(),
                        }],
                    },
                );
                let _ = ctx.broadcaster.send(broadcast_packet);

                Some(packet)
            }

            ContenuPaquet::SetBlock { x, y, z, block_id } => {
                ctx.state.set_block(*x, *y, *z, *block_id);

                let broadcast_packet = Paquet::new(
                    TypePaquet::SetBlock,
                    ContenuPaquet::SetBlock {
                        x: *x,
                        y: *y,
                        z: *z,
                        block_id: *block_id,
                    },
                );
                let _ = ctx.broadcaster.send(broadcast_packet);

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

            ContenuPaquet::GamemodeChange { player_id, gamemode } => {
                ctx.state.set_player_gamemode(*player_id, gamemode.clone());
                Some(packet)
            }

            _ => {
                log_server!("Joueur {}: paquet non géré, éjection.", ctx.player_id);
                None
            }
        }
    }
}
