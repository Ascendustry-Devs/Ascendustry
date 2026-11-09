use game::world::world_position::WorldPosition;

pub trait CollisionWorld {
    fn is_empty(&self) -> bool;
    fn is_block_solid(&self, position: &WorldPosition) -> bool;
}
