use game::world::{
    data::block::{BlockId, BlockInstance},
    local_position::LocalPosition,
    modified_chunk::{ModifiedChunk, ModifiedWorld},
    world_position::WorldPosition,
};

#[test]
fn test_modified_chunk() {
    let mut mc = ModifiedChunk::new();
    let coords = LocalPosition::new(5, 10, 15);
    let stone = BlockInstance::new(BlockId(3));

    assert!(mc.get_block_at(&coords).is_none());
    mc.set_block_at(&coords, stone);
    assert_eq!(mc.get_block_at(&coords), Some(&stone));
}

#[test]
fn test_modified_world() {
    let mut mw = ModifiedWorld::new();
    let stone = BlockInstance::new(BlockId(3));

    mw.set_block_at(&WorldPosition::new(5, 10, 15), stone);
    assert_eq!(mw.get_block_at(&WorldPosition::new(5, 10, 15)), Some(&stone));
    assert!(mw.get_block_at(&WorldPosition::new(0, 0, 0)).is_none());
}
