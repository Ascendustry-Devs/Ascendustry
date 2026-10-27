use engine::core::frame::EngineFrameData;
use game::{
    inventory::ItemStack,
    world::data::block::{BlockData, BlockManager},
};

use crate::{
    player::{player::Player, remote_players::RemotePlayersManager},
    world::world::World,
};

pub struct WorldBrowser<'a> {
    world: &'a World,
    registry: &'a BlockManager,
}

pub struct PlayerBrowser<'a> {
    me: &'a Player,
    them: &'a RemotePlayersManager,
}

pub struct TechnicalBrowser<'a> {
    frame: &'a EngineFrameData,
}

pub struct UiContext<'a> {
    pub world: WorldBrowser<'a>,
    pub players: PlayerBrowser<'a>,
    pub system: TechnicalBrowser<'a>,
}

impl<'a> WorldBrowser<'a> {
    pub fn browse_block_at(&self, x: i32, y: i32, z: i32) -> Option<&BlockData> {
        let id = self.world.get_block_from_xyz(x, y, z).get_block_id();
        self.registry.get_block_by_id(id)
    }
}

impl<'a> PlayerBrowser<'a> {
    pub fn browse_slot(&self, slot: usize) -> Option<&ItemStack> {
        self.me.state.inventory.get_slot(slot)
    }

    pub fn browse_total_player_count(&self) -> u32 {
        1 + self.them.get_player_count()
    }
}

impl<'a> TechnicalBrowser<'a> {
    pub fn browse_dt(&self) -> f32 {
        self.frame.dt
    }
}
