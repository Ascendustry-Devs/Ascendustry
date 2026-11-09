use crate::identity::IdentityRegistry;
use crate::metrics::ServerMetrics;
use crate::persistence::{PlayerSave, SaveData, SaveWorld};
use crate::player::PlayerRegistry;
use crate::world::ServerWorld;
use game::assets::resolve_asset_path;
use game::constants::{SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z};
use game::inventory::{Inventory, SlotData};
use game::player::{PlayerGameMode, PlayerTransformation};
use game::types::{Position, Rotation};
use game::world::data::block::BlockInstance;
use game::world::world_position::WorldPosition;
use game::world::ChunkStorage;
use game::world::World;
use network::messages::{BroadcastMessage, ChunkData, ContenuPaquet, Paquet, TypePaquet};
use physics::position::{find_safe_spawn_point, is_position_free};
use physics::validator::is_movement_plausible;
use project_core::log_server;
use rand::random;
use std::path::Path;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

pub struct AppState {
    world: RwLock<ServerWorld>,
    players: RwLock<PlayerRegistry>,
    identity: RwLock<IdentityRegistry>,
    pub metrics: ServerMetrics,
}

impl AppState {
    pub fn new(seed: u32) -> Self {
        Self::with_blocks_path(seed, resolve_asset_path("blocks/"), resolve_asset_path("items/"))
    }

    pub fn from_save(data: SaveData) -> Self {
        let modif_count = data.world.chunks.len();
        log_server!("import_save: {} chunks modifiés chargés (seed={})", modif_count, data.seed);

        let identity = data.identity;
        let mut players = PlayerRegistry::new();
        players.set_players(data.players);
        let mut world = ServerWorld::new(data.seed, resolve_asset_path("blocks/"), resolve_asset_path("items/"));
        world.modifications = data.world.into();

        let players = RwLock::new(players);
        let world = RwLock::new(world);
        let identity = RwLock::new(identity);

        Self {
            world,
            players,
            identity,
            metrics: ServerMetrics::new(),
        }
    }

    pub fn with_blocks_path(seed: u32, blocks_path: impl AsRef<Path>, items_path: impl AsRef<Path>) -> Self {
        Self {
            world: RwLock::new(ServerWorld::new(seed, blocks_path, items_path)),
            players: RwLock::new(PlayerRegistry::new()),
            identity: RwLock::new(IdentityRegistry::new()),
            metrics: ServerMetrics::new(),
        }
    }

    pub fn get_random_seed() -> u32 {
        random::<u32>()
    }

    pub async fn init_seed(&self, seed: u32) {
        self.world.write().await.set_seed(seed);
    }

    pub async fn get_seed(&self) -> u32 {
        self.world.read().await.get_seed()
    }

    pub async fn kick_player(&self, id: &u64, reason: &str) -> bool {
        self.players.write().await.kick(id, reason)
    }

    pub async fn add_player(&self, id: u64, username: String) {
        self.players.write().await.add(id, username);
        let world = self.world.write().await;
        let position = Position::new(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        let safe_position = find_safe_spawn_point(&*world, &position);
        drop(world);
        let safe_rotation = Rotation::new(0.0, 0.0);
        let mut player = self.players.write().await;
        player.update_position(id, safe_position, safe_rotation);
        player.set_last_valid_transformation(id, safe_position, safe_rotation);
    }

    pub async fn remove_player(&self, id: &u64) {
        self.players.write().await.remove(id);
        let keep = self.players.read().await.all_required_chunks();
        self.world.write().await.retain_chunks(&keep);
    }

    pub async fn get_all_players_vec(&self) -> Option<Vec<crate::player::Player>> {
        self.players.read().await.get_all()
    }

    pub async fn get_player(&self, id: u64) -> Option<crate::player::Player> {
        self.players.read().await.get(&id).cloned()
    }

    pub async fn get_player_position(&self, id: u64) -> Option<Position> {
        let a = self.players.read().await.get(&id).cloned();
        a.map(|p| p.position)
    }

    pub async fn get_player_rotation(&self, id: u64) -> Option<Rotation> {
        let a = self.players.read().await.get(&id).cloned();
        a.map(|p| p.rotation)
    }

    pub async fn set_block(&self, position: &WorldPosition, block: BlockInstance) {
        let _ = self.world.write().await.set_block(position, block);
    }

    pub async fn update_player_position(&self, id: u64, position: Position, rotation: Rotation) {
        let chunk_position = self.players.write().await.update_position(id, position, rotation);
        let required = ServerWorld::get_required_chunks(&chunk_position);

        self.players.write().await.set_player_chunks(id, required.clone());

        let missing = {
            let generated = self.world.read().await;
            required
                .iter()
                .filter(|c| !generated.chunks().contains_key(*c))
                .cloned()
                .collect::<Vec<_>>()
        };
        if !missing.is_empty() {
            self.world.write().await.generate_missing(&missing);
        }

        let keep = self.players.read().await.all_required_chunks();
        self.world.write().await.retain_chunks(&keep);
    }

    pub async fn update_inventory(&self, id: u64, inventory: Vec<SlotData>) {
        let mut players = self.players.write().await;
        players.update_inventory(id, inventory.clone());
    }

    pub async fn set_inventory(&self, id: u64, inventory: Inventory) {
        let mut players = self.players.write().await;
        players.set_inventory(id, inventory.clone());
    }

    pub async fn set_player_gamemode(&self, id: u64, gamemode: PlayerGameMode) {
        let mut players = self.players.write().await;
        players.update_gamemode(id, gamemode);
        if gamemode != PlayerGameMode::Spectator {
            let position = players
                .get(&id)
                .map_or(Position::new(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z), |p| {
                    p.position
                });
            let world = self.world.write().await;
            let surface = find_safe_spawn_point(&*world, &position);
            drop(world);
            if let Some(player) = players.get_mut(&id) {
                player.position = surface;
                player.last_valid_position = surface;
            }
        }
    }

    pub async fn export_save(&self) -> SaveData {
        let world = SaveWorld::from(&self.world.read().await.modifications);
        let players = self.players.read().await.get_all().unwrap_or_default();
        let identity = self.identity.read().await.clone();
        let modif_count = world.chunks.len();
        log_server!(
            "export_save: {} chunks modifiés sauvegardés (seed={})",
            modif_count,
            self.world.read().await.seed()
        );
        SaveData::new(self.world.read().await.seed(), world, players, identity)
    }

    // Le guard cycle permet de vérifier si les positions des joueurs sont valides et de les déplacer si nécessaire.
    pub async fn run_guard_cycle(&self, broadcaster: &broadcast::Sender<BroadcastMessage>) {
        let start = std::time::Instant::now();
        // Phase 1 : évaluation sous read lock
        let evaluations = {
            let mut evals = Vec::new();

            for (_, player) in self.players.read().await.iter() {
                if player.gamemode == PlayerGameMode::Spectator {
                    continue;
                }

                let valid = is_position_free(&*self.world.read().await, &player.position);

                let plausible = is_movement_plausible(&player.last_valid_position, &player.position, 0.2);

                evals.push((
                    player.id,
                    player.position,
                    player.last_valid_position,
                    player.last_valid_rotation,
                    valid && plausible,
                ));
            }

            evals
        };

        // Phase 2 : mutations sous write lock
        let mut corrections = Vec::new();
        let mut players = self.players.write().await;
        for (id, cur_pos, last_pos, last_rot, ok) in evaluations {
            if ok {
                if let Some(player) = players.get_mut(&id) {
                    player.last_valid_position = cur_pos;
                    player.last_valid_rotation = player.rotation;
                }
            } else {
                if let Some(player) = players.get_mut(&id) {
                    player.position = last_pos;
                    player.rotation = last_rot;
                }

                corrections.push(PlayerTransformation {
                    player_id: id,
                    position: last_pos,
                    rotation: last_rot,
                });
            }
        }
        drop(players);

        if !corrections.is_empty() {
            let packet = Paquet::new(
                TypePaquet::GuardCorrection,
                ContenuPaquet::GuardCorrection { data: corrections },
            );
            let _ = broadcaster.send(BroadcastMessage::All(packet));
        }

        let elapsed = start.elapsed().as_nanos() as u64;
        self.metrics.record_guard_cycle(elapsed);
    }
    pub async fn get_chunk_count(&self) -> usize {
        self.world.read().await.chunks().len()
    }

    pub async fn get_modified_count(&self) -> usize {
        self.world.read().await.modifications.chunks().len()
    }

    pub async fn get_modified_chunks_data(&self) -> Vec<ChunkData> {
        let mut world = self.world.write().await;
        let missing: Vec<_> = world
            .modifications
            .chunks()
            .keys()
            .filter(|key| !world.chunks().contains_key(key))
            .cloned()
            .collect();
        if !missing.is_empty() {
            world.generate_missing(&missing);
        }

        world.collect_modified_chunks_data()
    }

    pub async fn check_identity(&self, player_id: u64, identity: &str) -> bool {
        self.identity.read().await.check(player_id, identity)
    }
    pub async fn register_identity(&self, player_id: u64, identity: String) {
        self.identity.write().await.register(player_id, identity);
    }

    pub async fn save_player_data(
        &self,
        player_unique_id: u64,
        position: Position,
        rotation: Rotation,
        gamemode: PlayerGameMode,
        inventory: Inventory,
    ) {
        self.identity.write().await.save_player_data(
            player_unique_id,
            PlayerSave {
                position,
                rotation,
                gamemode,
                inventory,
            },
        );
    }

    pub async fn take_saved_player_data(&self, player_unique_id: u64) -> Option<PlayerSave> {
        self.identity.write().await.take_player_data(player_unique_id)
    }

    pub async fn restore_player(
        &self,
        id: u64,
        position: Position,
        rotation: Rotation,
        gamemode: PlayerGameMode,
        inventory: Inventory,
    ) {
        let mut players = self.players.write().await;
        players.update_position(id, position, rotation);
        players.set_last_valid_transformation(id, position, rotation);
        players.update_gamemode(id, gamemode);
        players.set_inventory(id, inventory);
    }
}
