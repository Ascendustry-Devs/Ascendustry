use engine::{core::frame::EngineFrameData, gpu::textures::manager::TextureManager};
use game::{inventory::ItemStack, world::data::block::BlockData};

use crate::{
    player::{player::Player, remote_players::RemotePlayersManager},
    world::world::World,
};

pub struct WorldBrowser<'a> {
    world: &'a World,
}

pub struct PlayerBrowser<'a> {
    me: &'a Player,
    them: &'a RemotePlayersManager,
}

pub struct AssetBrowser<'a> {
    textures: &'a TextureManager,
}

pub struct TechnicalBrowser<'a> {
    frame: &'a EngineFrameData,
}

pub struct UiContext<'a> {
    pub world: WorldBrowser<'a>,
    pub players: PlayerBrowser<'a>,
    pub assets: AssetBrowser<'a>,
    pub system: TechnicalBrowser<'a>,
}

impl<'a> WorldBrowser<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn browse_block_at(&self, x: i32, y: i32, z: i32) -> Option<&BlockData> {
        let id = self.world.get_block_from_xyz(x, y, z).get_block_id();
        let registry = self.world.get_block_registry();
        registry.get_block_by_id(id)
    }
}

impl<'a> PlayerBrowser<'a> {
    pub fn new(me: &'a Player, them: &'a RemotePlayersManager) -> Self {
        Self { me, them }
    }

    pub fn browse_slot(&self, slot: usize) -> Option<&ItemStack> {
        self.me.state.inventory.get_slot(slot)
    }

    pub fn browse_selected_slot(&self) -> (usize, &ItemStack) {
        (
            self.me.state.selected_slot,
            self.browse_slot(self.me.state.selected_slot).unwrap(),
        )
    }

    pub fn browse_total_player_count(&self) -> u32 {
        1 + self.them.get_player_count()
    }
}

impl<'a> AssetBrowser<'a> {
    pub fn new(textures: &'a TextureManager) -> Self {
        Self { textures }
    }

    pub fn get_item_texture(&self, id: u32) -> u32 {
        todo!("Implémenter un Index des textures GPUs sur le TextureManager puis le connecter ici")
    }
}

impl<'a> TechnicalBrowser<'a> {
    pub fn new(frame: &'a EngineFrameData) -> Self {
        Self { frame }
    }

    pub fn browse_dt(&self) -> f32 {
        self.frame.dt
    }
}
