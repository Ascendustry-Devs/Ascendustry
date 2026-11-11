use std::sync::{Arc, RwLock};

use game::{
    assets::{block_loader::load_block_definitions, resolve_asset_path},
    constants::{
        horizontal_chunk_axis_count, vertical_chunk_axis_count, HORIZONTAL_RENDER_DISTANCE, HORIZONTAL_SIMULATION_DISTANCE,
        MAX_GENERATION_CHUNKS_IN_QUEUE, MAX_MESHING_CHUNKS_IN_QUEUE, RENDER_DISTANCE_CHUNK_COUNT,
        SIMULATION_DISTANCE_CHUNK_COUNT, VERTICAL_RENDER_DISTANCE, VERTICAL_SIMULATION_DISTANCE,
    },
    world::{
        chunk_position::ChunkPosition,
        data::{
            block::{BlockId, BlockInstance, BlockManager},
            chunk::{Chunk, ChunkData, ChunkState},
        },
        local_position::LocalPosition,
        world_position::WorldPosition,
    },
};

#[test]
fn test_chunk_coordinates() {
    let c: ChunkPosition = (&WorldPosition::new(35, 70, -5)).into();
    assert_eq!(c.x(), 1);
    assert_eq!(c.y(), 2);
    assert_eq!(c.z(), -1);

    let l: LocalPosition = (&WorldPosition::new(35, 70, -5)).into();
    assert_eq!(l.x(), 3);
    assert_eq!(l.y(), 6);
    assert_eq!(l.z(), 27);
}

#[test]
fn test_chunk_get_set_block() {
    let blocks = vec![BlockInstance::air(); 32768];
    let mut chunk = Chunk {
        blocks,
        x: 0,
        y: 0,
        z: 0,
    };

    let stone = BlockInstance::new(BlockId(3));
    let position = LocalPosition::new(5, 10, 15);
    chunk.set_block(&position, stone);
    assert_eq!(chunk.get_block(&position), stone);
}

#[test]
fn test_chunk_range() {
    let range = Chunk::get_cube_chunk_range(&ChunkPosition::new(0, 0, 0), 5, 3);
    assert_eq!(range, [-2, 2, -1, 1, -2, 2]);
}

#[test]
fn test_chunk_keys() {
    let keys = Chunk::get_cube_chunk_keys(0, 1, 0, 1, 0, 1);
    assert_eq!(keys.len(), 8);
    let keys_set = Chunk::get_cube_chunk_keys_set(0, 1, 0, 1, 0, 1);
    assert_eq!(keys_set.len(), 8);
}

#[test]
fn test_chunk_generation_deterministic() {
    // On charge les vraies définitions de blocs : la génération dépend du
    // contenu de `assets/biomes.json`, qui référence des blocs variables
    // (sable, neige, eau...). Hardcoder une liste de blocs rendrait ce
    // test fragile et leaky (il échouerait dès qu'un biome Evolue).
    let mut bm = BlockManager::new();
    let blocks = load_block_definitions(resolve_asset_path("blocks/")).expect("Failed to load block definitions");
    assert!(!blocks.is_empty());
    for block in blocks {
        bm.register(block);
    }
    let bm = Arc::new(RwLock::new(bm));
    let position = ChunkPosition::new(0, 2, 0);
    let chunk1 = Chunk::generate(Arc::clone(&bm), &position, 42);
    let chunk2 = Chunk::generate(Arc::clone(&bm), &position, 42);
    assert_eq!(chunk1.blocks, chunk2.blocks);
}

#[test]
fn test_chunk_checksum() {
    let blocks = vec![BlockInstance::air(); 32768];
    let mut chunk = Chunk {
        blocks,
        x: 0,
        y: 0,
        z: 0,
    };

    let checksum1 = chunk.compute_checksum();
    chunk.set_block(&LocalPosition::zero(), BlockInstance::new(BlockId(1)));
    let checksum2 = chunk.compute_checksum();
    assert_ne!(checksum1, checksum2);
}

#[test]
fn test_chunk_neighbors_from_block_pos() {
    let neighbors = Chunk::neighbors_from_block_pos(&WorldPosition::new(0, 0, 0));
    assert!(neighbors.contains(&(-1, 0, 0)));
    assert!(neighbors.contains(&(0, -1, 0)));
    assert!(neighbors.contains(&(0, 0, -1)));

    let inner = Chunk::neighbors_from_block_pos(&WorldPosition::new(16, 16, 16));
    assert!(inner.is_empty());
}

#[test]
fn test_chunk_data_debug() {
    let chunk = Chunk {
        blocks: vec![BlockInstance::air(); 32768],
        x: 1,
        y: 2,
        z: 3,
    };
    let mut cd = ChunkData::new(chunk);
    assert_eq!(cd.get_debug_infos(), (ChunkState::Ready, true));

    cd.set_dirty();
    assert_eq!(cd.get_debug_infos(), (ChunkState::Ready, true));
}

#[test]
fn test_chunk_volume_constants_match_real_ranges() {
    // Les constantes d'axe doivent refléter exactement la taille du cube
    // réellement produit par `get_cube_chunk_range`.
    let render = Chunk::get_cube_chunk_range(
        &ChunkPosition::new(0, 0, 0),
        HORIZONTAL_RENDER_DISTANCE,
        VERTICAL_RENDER_DISTANCE,
    );
    assert_eq!(
        render[1] - render[0] + 1,
        horizontal_chunk_axis_count(HORIZONTAL_RENDER_DISTANCE) as i32
    );
    assert_eq!(
        render[3] - render[2] + 1,
        vertical_chunk_axis_count(VERTICAL_RENDER_DISTANCE) as i32
    );
    assert_eq!(
        render[5] - render[4] + 1,
        horizontal_chunk_axis_count(HORIZONTAL_RENDER_DISTANCE) as i32
    );

    let simulation = Chunk::get_cube_chunk_range(
        &ChunkPosition::new(0, 0, 0),
        HORIZONTAL_SIMULATION_DISTANCE,
        VERTICAL_SIMULATION_DISTANCE,
    );
    assert_eq!(
        simulation[1] - simulation[0] + 1,
        horizontal_chunk_axis_count(HORIZONTAL_SIMULATION_DISTANCE) as i32
    );
    assert_eq!(
        simulation[3] - simulation[2] + 1,
        vertical_chunk_axis_count(VERTICAL_SIMULATION_DISTANCE) as i32
    );

    // Volumes attendus : rendu 15x5x15 = 1125, simulation 17x7x17 = 2023.
    assert_eq!(RENDER_DISTANCE_CHUNK_COUNT, 1125);
    assert_eq!(SIMULATION_DISTANCE_CHUNK_COUNT, 2023);

    // Les files doivent pouvoir contenir l'intégralité du volume concerné.
    assert_eq!(MAX_MESHING_CHUNKS_IN_QUEUE, RENDER_DISTANCE_CHUNK_COUNT as u32);
    assert_eq!(MAX_GENERATION_CHUNKS_IN_QUEUE, SIMULATION_DISTANCE_CHUNK_COUNT as u32);
}

#[test]
fn test_simulation_cube_contains_render_cube() {
    // Le cube de simulation doit contenir le cube de rendu, ainsi que la
    // coquille de voisins nécessaire au maillage (1 chunk de marge).
    let render = Chunk::get_cube_chunk_range(
        &ChunkPosition::new(0, 0, 0),
        HORIZONTAL_RENDER_DISTANCE,
        VERTICAL_RENDER_DISTANCE,
    );
    let simulation = Chunk::get_cube_chunk_range(
        &ChunkPosition::new(0, 0, 0),
        HORIZONTAL_SIMULATION_DISTANCE,
        VERTICAL_SIMULATION_DISTANCE,
    );

    assert!(simulation[0] < render[0]);
    assert!(simulation[1] > render[1]);
    assert!(simulation[2] < render[2]);
    assert!(simulation[3] > render[3]);
    assert!(simulation[4] < render[4]);
    assert!(simulation[5] > render[5]);
}
