use std::sync::Arc;
use std::time::SystemTime;

use crate::{persistence::PersistenceService, state::AppState};
use game::player::PlayerTransformation;
use network::messages::{BroadcastMessage, *};
use project_core::{log_err_server, log_server};
use tokio::sync::broadcast;

pub struct HandlerContext {
    pub player_id: u64,
    pub state: Arc<AppState>,
    pub broadcaster: broadcast::Sender<BroadcastMessage>,
    pub persistence: Arc<PersistenceService>,
}

pub trait PacketHandler: Send + Sync {
    fn handle(&self, packet: Paquet, ctx: &HandlerContext) -> impl std::future::Future<Output = Option<Paquet>> + Send;
}

pub struct ProductionHandler;

impl PacketHandler for ProductionHandler {
    async fn handle(&self, packet: Paquet, ctx: &HandlerContext) -> Option<Paquet> {
        match &packet.contenu {
            ContenuPaquet::DonneesConnexion { version, username, .. } => {
                log_server!("Joueur {}: connexion avec la version {}.", username, version);
                Some(packet)
            }

            ContenuPaquet::PlayerTransformation { data } => {
                ctx.state
                    .update_player_position(data.player_id, data.position.clone(), data.rotation.clone())
                    .await;

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
                let _ = ctx.broadcaster.send(BroadcastMessage::All(broadcast_packet));

                Some(packet)
            }

            ContenuPaquet::SetBlock { x, y, z, block_id } => {
                ctx.state.set_block(*x, *y, *z, *block_id).await;

                let broadcast_packet = Paquet::new(
                    TypePaquet::SetBlock,
                    ContenuPaquet::SetBlock {
                        x: *x,
                        y: *y,
                        z: *z,
                        block_id: *block_id,
                    },
                );
                let _ = ctx.broadcaster.send(BroadcastMessage::AllExcept {
                    player_id: ctx.player_id,
                    paquet: broadcast_packet,
                });

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
                ctx.state.set_player_gamemode(*player_id, gamemode.clone()).await;
                Some(packet)
            }

            ContenuPaquet::SaveRequest => {
                log_server!("Sauvegarde demandée par le joueur {}.", ctx.player_id);
                let state = Arc::clone(&ctx.state);
                let persistence = Arc::clone(&ctx.persistence);
                tokio::spawn(async move {
                    let data = state.export_save().await;
                    if let Err(e) = persistence.save(data).await {
                        log_err_server!("Échec de la sauvegarde : {}", e);
                    }
                });
                Some(packet)
            }

            _ => {
                log_server!("Joueur {}: paquet non géré, éjection.", ctx.player_id);
                None
            }
        }
    }
}
