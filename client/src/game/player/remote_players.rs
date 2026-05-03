use shared::network::messages::PlayerTransformation;
use std::collections::HashMap;

pub struct RemotePlayer {
    pub player_id: u64,
    pub position: (f32, f32, f32),
    pub rotation: (f32, f32),
    pub mesh_id: Option<u32>,
}

pub struct RemotePlayersManager {
    players: HashMap<u64, RemotePlayer>,
}

impl RemotePlayersManager {
    pub fn new() -> Self {
        Self { players: HashMap::new() }
    }

    pub fn update(&mut self, transforms: Vec<PlayerTransformation>, my_id: u64) {
        for t in transforms {
            if t.player_id != my_id {
                self.players.insert(
                    t.player_id,
                    RemotePlayer {
                        player_id: t.player_id,
                        position: (t.position.x, t.position.y, t.position.z),
                        rotation: (t.rotation.x, t.rotation.y),
                        mesh_id: None,
                    },
                );
            }
        }
    }

    pub fn get_all(&self) -> Vec<&RemotePlayer> {
        self.players.values().collect()
    }

    pub fn get_all_mut(&mut self) -> Vec<&mut RemotePlayer> {
        self.players.values_mut().collect()
    }
}
