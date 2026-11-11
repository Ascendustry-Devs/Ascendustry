use game::world::data::block::{BlockData, BlockManager};

#[test]
fn test_block_manager_default() {
    let mut bm = BlockManager::new();
    for id in &["air", "stone", "dirt", "grass"] {
        bm.register(BlockData {
            id: None,
            id_str: id.to_string(),
            name: id.to_string(),
            solid: true,
            hardness: 0.5,
            texture_path: None,
            render_mode: "opaque".to_string(),
            drop: None,
            has_item: true,
        });
    }
    assert_eq!(bm.block_count(), 4);
    let block = bm.get_block_by_string("dirt".to_string());
    assert!(block.is_some());
    assert_eq!(block.unwrap().get_id_str(), "dirt");
}
