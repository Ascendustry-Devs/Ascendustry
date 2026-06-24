use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde::Serialize;

use crate::persistence::PlayerSave;

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityRegistry {
    identity: FxHashMap<u64, String>,
    saves: FxHashMap<u64, PlayerSave>,
}

impl Default for IdentityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityRegistry {
    pub fn new() -> Self {
        Self {
            identity: FxHashMap::default(),
            saves: FxHashMap::default(),
        }
    }

    pub fn register(&mut self, player_id: u64, name: String) {
        self.identity.insert(player_id, name);
    }
    pub fn check(&self, player_id: u64, name: &str) -> bool {
        self.identity.get(&player_id).is_none_or(|registered| registered == name)
    }
    pub fn entries(&self) -> Vec<(&u64, &String)> {
        self.identity.iter().collect()
    }
    pub fn load(&mut self, entries: Vec<(&u64, &String)>) {
        for (player_id, name) in entries {
            self.identity.insert(*player_id, name.to_string());
        }
    }
    pub fn save_player_data(&mut self, player_id: u64, save: PlayerSave) {
        self.saves.insert(player_id, save);
    }
    pub fn take_player_data(&mut self, player_id: u64) -> Option<PlayerSave> {
        self.saves.remove(&player_id)
    }
}
