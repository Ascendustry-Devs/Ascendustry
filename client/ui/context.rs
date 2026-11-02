use engine::{core::frame::EngineFrameData, gpu::textures::id::TextureId};
use game::{
    inventory::{item_manager::ItemId, Inventory, ItemStack},
    world::data::block::BlockData,
};

use crate::{
    player::{player::Player, remote_players::RemotePlayersManager},
    systems::texture_registry::TextureRegistry,
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
    textures: &'a TextureRegistry,
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

impl<'a> UiContext<'a> {
    pub fn new(
        world: &'a World,
        me: &'a Player,
        them: &'a RemotePlayersManager,
        textures: &'a TextureRegistry,
        frame: &'a EngineFrameData,
    ) -> UiContext<'a> {
        UiContext {
            world: WorldBrowser::new(world),
            players: PlayerBrowser::new(me, them),
            assets: AssetBrowser::new(textures),
            system: TechnicalBrowser::new(frame),
        }
    }
}

impl<'a> WorldBrowser<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn browse_block_at(&self, x: i32, y: i32, z: i32) -> Option<&BlockData> {
        let id = self.world.get_block_from_xyz(x, y, z).get_block_id();
        let registry = self.world.get_block_registry();
        registry.get_block_by_id(&id)
    }
}

impl<'a> PlayerBrowser<'a> {
    pub fn new(me: &'a Player, them: &'a RemotePlayersManager) -> Self {
        Self { me, them }
    }

    pub fn browse_local_inventory(&self) -> &Inventory {
        &self.me.state.inventory
    }

    pub fn browse_slot(&self, slot: usize) -> Option<&ItemStack> {
        self.me.state.inventory.get_slot(slot)
    }

    pub fn browse_selected_slot(&self) -> (usize, Option<&ItemStack>) {
        (self.me.state.selected_slot, self.browse_slot(self.me.state.selected_slot))
    }

    pub fn browse_total_player_count(&self) -> u32 {
        1 + self.them.get_player_count()
    }
}

impl<'a> AssetBrowser<'a> {
    pub fn new(textures: &'a TextureRegistry) -> Self {
        Self { textures }
    }

    pub fn get_item_texture(&self, item_id: &ItemId) -> TextureId {
        self.textures
            .get_item_texture(item_id)
            .map_or_default(|texture_id| texture_id.clone())
        // return 0;
        // todo!("Implémenter un Index des textures GPUs sur le TextureManager puis le connecter ici")
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
