use engine::{core::frame::EngineFrameData, gpu::textures::id::TextureId};
use game::{
    inventory::{item_manager::ItemId, Inventory, ItemStack},
    world::{data::block::BlockData, world_position::WorldPosition, World},
};

use crate::{
    player::{remote_players::RemotePlayersManager, Player},
    systems::texture_registry::TextureRegistry,
    world::ClientWorld,
};

pub struct WorldBrowser<'a> {
    world: &'a ClientWorld,
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
        world: &'a ClientWorld,
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
    pub fn new(world: &'a ClientWorld) -> Self {
        Self { world }
    }

    pub fn browse_block_at(&self, position: &WorldPosition) -> Option<BlockData> {
        let Some(id) = self.world.get_block(position).map(|block| block.get_block_id()) else {
            return None;
        };
        let registry = self.world.block_manager().try_read().ok()?;
        return registry.get_block_by_id(&id).cloned();
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
            .map_or_default(|texture_id| *texture_id)
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
