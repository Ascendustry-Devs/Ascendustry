use game::world::{chunk_position::ChunkPosition, world_position::WorldPosition};

#[test]
fn test_global_position_to_chunk_pos() {
    let chunk_position: ChunkPosition = (&WorldPosition::new(5, 70, -3)).into();
    let (cx, cy, cz) = (&chunk_position).into();
    assert_eq!(cx, 0);
    assert_eq!(cy, 2);
    assert_eq!(cz, -1);
}
