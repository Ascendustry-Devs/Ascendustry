use crate::player::PlayerRegistry;
use crate::world::WorldState;
use shared::network::messages::{ContenuPaquet, Paquet, PlayerGameMode, PlayerTransformation, Position, Rotation, TypePaquet};
use shared::world::constants::{SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z};
use std::sync::RwLock;
use tokio::sync::broadcast;

pub struct AppState {
    inner: RwLock<ServerState>,
}

struct ServerState {
    world: WorldState,
    players: PlayerRegistry,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(ServerState {
                world: WorldState::new(),
                players: PlayerRegistry::new(),
            }),
        }
    }

    pub fn init_random_seed(&self) {
        let seed = rand::random::<u32>();
        self.inner.write().unwrap().world.set_seed(seed);
    }

    pub fn init_seed(&self, seed: u32) {
        self.inner.write().unwrap().world.set_seed(seed);
    }

    pub fn get_seed(&self) -> u32 {
        self.inner.read().unwrap().world.get_seed()
    }

    pub fn add_player(&self, id: u64, username: String) {
        let mut state = self.inner.write().unwrap();
        state.players.add(id, username);
        let safe_position = state
            .world
            .find_safe_spawn_point(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        let safe_rotation = Rotation { x: 0.0, y: 0.0 };
        state.players.update_position(id, safe_position.clone(), safe_rotation.clone());
        state.players.set_last_valid_transformation(id, safe_position, safe_rotation);
    }

    pub fn remove_player(&self, id: &u64) {
        let mut state = self.inner.write().unwrap();
        state.players.remove(id);
        let keep = state.players.all_required_chunks();
        state.world.retain_chunks(&keep);
    }

    pub fn get_all_players_vec(&self) -> Option<Vec<crate::player::Player>> {
        self.inner.read().unwrap().players.get_all()
    }

    pub fn set_block(&self, x: i32, y: i32, z: i32, block_id: u32) {
        self.inner.write().unwrap().world.set_block(x, y, z, block_id);
    }

    pub fn update_player_position(&self, id: u64, position: Position, rotation: Rotation) {
        let mut state = self.inner.write().unwrap();

        let chunk_pos = state.players.update_position(id, position, rotation);
        let required = WorldState::get_required_chunks(chunk_pos.0, chunk_pos.1, chunk_pos.2);

        state.players.set_player_chunks(id, required.clone());

        let missing: Vec<_> = required
            .iter()
            .filter(|c| !state.world.world_generated_chunks.contains_key(*c))
            .cloned()
            .collect();
        if !missing.is_empty() {
            state.world.generate_missing(&missing);
        }

        let keep = state.players.all_required_chunks();
        state.world.retain_chunks(&keep);
    }

    pub fn set_player_gamemode(&self, id: u64, gamemode: PlayerGameMode) {
        let mut state = self.inner.write().unwrap();
        state.players.update_gamemode(id, gamemode.clone());
        if gamemode != PlayerGameMode::Spectator {
            let (x, y, z) = state
                .players
                .get(&id)
                .map(|p| (p.position.x, p.position.y, p.position.z))
                .unwrap_or((0.0, 0.0, 0.0));
            if !state.world.is_position_free(x, y, z) {
                state.players.reset_to_last_valid_transformation(id);
            }
        }
    }

    // Le guard cycle permet de vérifier si les positions des joueurs sont valides et de les déplacer si nécessaire.
    pub fn run_guard_cycle(&self, broadcaster: &broadcast::Sender<Paquet>) {
        // Phase 1 : évaluation sous read lock
        let evaluations = {
            let state = self.inner.read().unwrap();
            let mut evals = Vec::new();

            for (_, player) in state.players.iter() {
                if player.gamemode == PlayerGameMode::Spectator {
                    continue;
                }

                let valid = state
                    .world
                    .is_position_free(player.position.x, player.position.y, player.position.z);

                let plausible = crate::game::validator::is_movement_plausible(&player.last_valid_position, &player.position, 0.2);

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
        let mut state = self.inner.write().unwrap();
        let mut corrections = Vec::new();

        for (id, cur_pos, last_pos, last_rot, ok) in evaluations {
            if ok {
                if let Some(player) = state.players.get_mut(&id) {
                    player.last_valid_position = cur_pos;
                    player.last_valid_rotation = player.rotation;
                }
            } else {
                if let Some(player) = state.players.get_mut(&id) {
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
            let _ = broadcaster.send(packet);
        }
    }
}
