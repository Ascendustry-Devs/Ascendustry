use game::world::data::block::{BlockId, BlockInstance};

#[test]
fn test_block_instance() {
    let air = BlockInstance::air();
    assert!(air.is_air());
    assert!(!air.is_solid());

    let stone = BlockInstance::new(BlockId(3));
    assert!(!stone.is_air());
    assert!(stone.is_solid());
    assert_eq!(stone.get_block_id(), BlockId(3));
}
