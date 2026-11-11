use std::sync::Arc;

use client::world::mesh_request::{MeshRequestAdd, MeshRequestQueue, MeshSnapshot};
use game::world::{
    chunk_position::ChunkPosition,
    data::{
        block::{BlockId, BlockInstance},
        chunk::Chunk,
    },
};

/// Construit un instantané identifiable : le premier bloc du chunk
/// principal porte l'identifiant `marker`.
fn make_snapshot(marker: BlockId) -> MeshSnapshot {
    let main = Arc::new(Chunk {
        blocks: vec![BlockInstance::new(marker); 32 * 32 * 32],
        x: 0,
        y: 0,
        z: 0,
    });
    MeshSnapshot {
        main: Arc::clone(&main),
        neg_x: None,
        pos_x: None,
        neg_y: None,
        pos_y: None,
        neg_z: None,
        pos_z: None,
    }
}

/// Identifiant porté par le premier bloc de l'instantané.
fn marker_of(snapshot: &MeshSnapshot) -> BlockId {
    snapshot.main.blocks[0].get_block_id()
}

#[test]
fn test_queue_preserves_insertion_order() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(2, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(2))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(3))));

    let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
    assert_eq!(
        coords,
        [(2, 0, 0), (0, 0, 0), (1, 0, 0)]
            .map(|(x, y, z)| ChunkPosition::new(x, y, z))
            .to_vec()
    );
}

#[test]
fn test_queue_upsert_replaces_snapshot_and_keeps_position() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(2))));

    // Ré-insertion du premier chunk avec un instantané plus récent.
    let inserted = queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(99))));
    assert!(!inserted, "le chunk était déjà présent");
    assert_eq!(queue.len(), 2, "aucun doublon ne doit être créé");

    let drained = queue.drain();
    assert_eq!(drained.len(), 2);
    assert_eq!(
        &drained[0].coords,
        &ChunkPosition::new(0, 0, 0),
        "la position doit être conservée"
    );
    // L'instantané doit avoir été remplacé.
    assert_eq!(marker_of(&drained[0].snapshot), BlockId(99));
}

#[test]
fn test_queue_sort_by_priority() {
    let mut queue = MeshRequestQueue::new();
    // Insertion volontairement dans le désordre.
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(5, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(2))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(3, 0, 0), make_snapshot(BlockId(3))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(4))));

    queue.sort_by_priority(&ChunkPosition::new(0, 0, 0));

    let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
    assert_eq!(
        coords,
        [(0, 0, 0), (1, 0, 0), (3, 0, 0), (5, 0, 0)]
            .map(|(x, y, z)| ChunkPosition::new(x, y, z))
            .to_vec()
    );
}

#[test]
fn test_queue_sort_by_priority_keeps_vertical_discount() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 4, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(4, 0, 0), make_snapshot(BlockId(2))));

    queue.sort_by_priority(&ChunkPosition::new(0, 0, 0));

    let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
    // 4 chunks en vertical (priorité 8) < 4 chunks à l'horizontale (16).
    assert_eq!(
        coords,
        [(0, 4, 0), (4, 0, 0)].map(|(x, y, z)| ChunkPosition::new(x, y, z)).to_vec()
    );
}

#[test]
fn test_queue_sort_by_priority_around_the_origin() {
    // Le joueur est à l'origine : les chunks sont répartis de part et
    // d'autre. L'ordre doit être correct, quel que soit le signe des
    // coordonnées (les distances sont signées, pas "unsigned").
    let mut queue = MeshRequestQueue::new();
    for coords in [(3, 0, 0), (-3, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 0)]
        .map(|(x, y, z)| ChunkPosition::new(x, y, z))
        .to_vec()
    {
        queue.insert(MeshRequestAdd::new(coords, make_snapshot(BlockId(1))));
    }

    queue.sort_by_priority(&ChunkPosition::new(0, 0, 0));

    let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
    // Distances : 0, puis 1 (1 et -1), puis 9 (3 et -3).
    // Le tri est stable : à distance égale, l'ordre d'insertion est gardé.
    assert_eq!(
        coords,
        [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (3, 0, 0), (-3, 0, 0)]
            .map(|(x, y, z)| ChunkPosition::new(x, y, z))
            .to_vec()
    );
}

#[test]
fn test_queue_sort_is_stable() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 1), make_snapshot(BlockId(2))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(-1, 0, 0), make_snapshot(BlockId(3))));

    queue.sort_by_priority(&ChunkPosition::new(0, 0, 0));

    let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
    // (1, 0, 0) et (-1, 0, 0) sont à distance 1 : le tri étant stable,
    // leur ordre d'insertion est conservé. (1, 0, 1) est à distance 2.
    assert_eq!(
        coords,
        [(1, 0, 0), (-1, 0, 0), (1, 0, 1)]
            .map(|(x, y, z)| ChunkPosition::new(x, y, z))
            .to_vec()
    );
}

#[test]
fn test_queue_remove_and_contains() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(2))));

    assert!(queue.contains(&ChunkPosition::new(0, 0, 0)));
    assert!(queue.remove(&ChunkPosition::new(0, 0, 0)).is_some());
    assert!(!queue.contains(&ChunkPosition::new(0, 0, 0)));
    assert_eq!(queue.len(), 1);
    assert!(queue.remove(&ChunkPosition::new(9, 9, 9)).is_none());
}

#[test]
fn test_queue_retain() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(1))));
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(1, 0, 0), make_snapshot(BlockId(2))));

    queue.retain(|request| request.coords.x() == 1);

    assert_eq!(queue.len(), 1);
    assert!(queue.contains(&ChunkPosition::new(1, 0, 0)));
    assert!(!queue.contains(&ChunkPosition::new(0, 0, 0)));
}

#[test]
fn test_queue_drain_empties_the_queue() {
    let mut queue = MeshRequestQueue::new();
    queue.insert(MeshRequestAdd::new(ChunkPosition::new(0, 0, 0), make_snapshot(BlockId(1))));
    assert_eq!(queue.drain().len(), 1);
    assert!(queue.is_empty());
    assert_eq!(queue.drain().len(), 0);
}
