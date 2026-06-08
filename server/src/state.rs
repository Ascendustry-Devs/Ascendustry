use crate::identity::IdentityRegistry;
use crate::persistence::{PlayerSave, SaveData, SaveWorld};
use crate::player::PlayerRegistry;
use crate::world::WorldState;
use game::constants::{SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z};
use network::messages::{
    BroadcastMessage, ChunkData, ContenuPaquet, Paquet, PlayerGameMode, PlayerTransformation, Position, Rotation, TypePaquet,
};
use physics::position::{find_safe_spawn_point, is_position_free};
use physics::validator::is_movement_plausible;
use project_core::log_server;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

pub struct AppState {
    world: RwLock<WorldState>,
    players: RwLock<PlayerRegistry>,
    identity: RwLock<IdentityRegistry>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            world: RwLock::new(WorldState::new()),
            players: RwLock::new(PlayerRegistry::new()),
            identity: RwLock::new(IdentityRegistry::new()),
        }
    }

    pub async fn init_random_seed(&self) {
        let seed = rand::random::<u32>();
        self.world.write().await.set_seed(seed);
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
        let (sx, sy, sz) = find_safe_spawn_point(&*world, SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        let safe_position = Position { x: sx, y: sy, z: sz };
        let safe_rotation = Rotation { x: 0.0, y: 0.0 };
        let mut player = self.players.write().await;
        player.update_position(id, safe_position.clone(), safe_rotation.clone());
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
        a.map(|p| p.position.clone())
    }

    pub async fn get_player_rotation(&self, id: u64) -> Option<Rotation> {
        let a = self.players.read().await.get(&id).cloned();
        a.map(|p| p.rotation.clone())
    }

    pub async fn set_block(&self, x: i32, y: i32, z: i32, block_id: u32) {
        self.world.write().await.set_block(x, y, z, block_id);
    }

    pub async fn update_player_position(&self, id: u64, position: Position, rotation: Rotation) {
        let chunk_pos = self.players.write().await.update_position(id, position, rotation);
        let required = WorldState::get_required_chunks(chunk_pos.0, chunk_pos.1, chunk_pos.2);

        self.players.write().await.set_player_chunks(id, required.clone());

        let generated = self.world.read().await;
        let missing: Vec<_> = required
            .iter()
            .filter(|c| !generated.world_generated_chunks.contains_key(*c))
            .cloned()
            .collect();
        if !missing.is_empty() {
            self.world.write().await.generate_missing(&missing);
        }

        let keep = self.players.read().await.all_required_chunks();
        self.world.write().await.retain_chunks(&keep);
    }

    pub async fn set_player_gamemode(&self, id: u64, gamemode: PlayerGameMode) {
        let mut players = self.players.write().await;
        players.update_gamemode(id, gamemode.clone());
        if gamemode != PlayerGameMode::Spectator {
            let (x, y, z) = players.get(&id).map(|p| (p.position.x, p.position.y, p.position.z)).unwrap_or((
                SPAWN_POSITION_X,
                SPAWN_POSITION_Y,
                SPAWN_POSITION_Z,
            ));
            let world = self.world.write().await;
            let (sx, sy, sz) = find_safe_spawn_point(&*world, x, y, z);
            let surface = Position { x: sx, y: sy, z: sz };
            if let Some(player) = players.get_mut(&id) {
                player.position = surface.clone();
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
            self.world.read().await.seed
        );
        SaveData::new(self.world.read().await.seed, world, players, identity)
    }

    pub async fn import_save(&self, data: SaveData) {
        let modif_count = data.world.chunks.len();
        log_server!("import_save: {} chunks modifiés chargés (seed={})", modif_count, data.seed);
        self.world.write().await.seed = data.seed;
        self.world.write().await.world_generated_chunks.clear();
        self.world.write().await.modifications = data.world.into();
        self.players.write().await.set_players(data.players);
        *self.identity.write().await = data.identity;
    }

    // Le guard cycle permet de vérifier si les positions des joueurs sont valides et de les déplacer si nécessaire.
    pub async fn run_guard_cycle(&self, broadcaster: &broadcast::Sender<BroadcastMessage>) {
        // Phase 1 : évaluation sous read lock
        let evaluations = {
            let mut evals = Vec::new();

            for (_, player) in self.players.read().await.iter() {
                if player.gamemode == PlayerGameMode::Spectator {
                    continue;
                }

                let valid = is_position_free(&*self.world.read().await, player.position.x, player.position.y, player.position.z);

                let plausible = is_movement_plausible(
                    player.last_valid_position.x,
                    player.last_valid_position.y,
                    player.last_valid_position.z,
                    player.position.x,
                    player.position.y,
                    player.position.z,
                    0.2,
                );

                evals.push((
                    player.id,
                    player.position.clone(),
                    player.last_valid_position.clone(),
                    player.last_valid_rotation.clone(),
                    valid && plausible,
                ));
            }

            evals
        };

        // Phase 2 : mutations sous write lock
        let mut corrections = Vec::new();

        for (id, cur_pos, last_pos, last_rot, ok) in evaluations {
            if ok {
                if let Some(player) = self.players.write().await.get_mut(&id) {
                    player.last_valid_position = cur_pos;
                    player.last_valid_rotation = player.rotation;
                }
            } else {
                if let Some(player) = self.players.write().await.get_mut(&id) {
                    player.position = last_pos.clone();
                    player.rotation = last_rot;
                }

                corrections.push(PlayerTransformation {
                    player_id: id,
                    position: last_pos,
                    rotation: last_rot,
                });
            }
        }

        if !corrections.is_empty() {
            let packet = Paquet::new(TypePaquet::GuardCorrection, ContenuPaquet::GuardCorrection { data: corrections });
            let _ = broadcaster.send(BroadcastMessage::All(packet));
        }
    }
    pub async fn get_chunk_count(&self) -> usize {
        self.world.read().await.world_generated_chunks.len()
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
            .filter(|key| !world.world_generated_chunks.contains_key(key))
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

    pub async fn save_player_data(&self, player_unique_id: u64, position: Position, rotation: Rotation, gamemode: PlayerGameMode) {
        self.identity.write().await.save_player_data(
            player_unique_id,
            PlayerSave {
                position,
                rotation,
                gamemode,
            },
        );
    }

    pub async fn take_saved_player_data(&self, player_unique_id: u64) -> Option<PlayerSave> {
        self.identity.write().await.take_player_data(player_unique_id)
    }

    pub async fn restore_player(&self, id: u64, position: Position, rotation: Rotation, gamemode: PlayerGameMode) {
        let mut players = self.players.write().await;
        players.update_position(id, position.clone(), rotation.clone());
        players.set_last_valid_transformation(id, position, rotation);
        players.update_gamemode(id, gamemode);
    }
}
