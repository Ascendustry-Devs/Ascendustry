#[cfg(test)]
mod tests {
    use crate::constants::DIRECT_NORMALS_3D;
    use crate::world::data::block::{BlockInstance, BlockManager, BlockType};
    use crate::world::data::chunk::{global_position_to_chunk_pos, Chunk, ChunkData, ChunkState, IntraChunkCoords};
    use crate::world::modified_chunk::{ModifiedChunk, ModifiedWorld};
    use crate::world::raycast::voxel_raycast;

    #[test]
    fn test_block_instance() {
        let air = BlockInstance::air();
        assert!(air.is_air());
        assert!(!air.is_solid());

        let stone = BlockInstance::new(3);
        assert!(!stone.is_air());
        assert!(stone.is_solid());
        assert_eq!(stone.to_bits(), 3);
    }

    #[test]
    fn test_block_type_from_id() {
        assert_eq!(BlockType::from_id(0) as u32, 0);
        assert_eq!(BlockType::from_id(3) as u32, 3);
        assert_eq!(BlockType::from_id(99) as u32, 2);
    }

    #[test]
    fn test_block_manager_default() {
        let bm = BlockManager::default();
        assert_eq!(bm.block_count(), 4);
        let block = bm.get_block_by_string("dirt".to_string());
        assert!(block.is_some());
        assert_eq!(block.unwrap().get_id_str(), "dirt");
    }

    #[test]
    fn test_chunk_coordinates() {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(35, 70, -5);
        assert_eq!(cx, 1);
        assert_eq!(cy, 2);
        assert_eq!(cz, -1);

        let (lx, ly, lz) = Chunk::local_coords_from_world(35, 70, -5);
        assert_eq!(lx, 3);
        assert_eq!(ly, 6);
        assert_eq!(lz, 27);
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

        let stone = BlockInstance::new(3);
        chunk.set_block_from_xyz(5, 10, 15, stone);
        assert_eq!(chunk.get_block_from_xyz(5, 10, 15), stone);
    }

    #[test]
    fn test_chunk_range() {
        let range = Chunk::get_cube_chunk_range((0, 0, 0), 5, 3);
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
    fn test_global_position_to_chunk_pos() {
        let ((cx, cy, cz), _intra) = global_position_to_chunk_pos(5, 70, -3);
        assert_eq!(cx, 0);
        assert_eq!(cy, 2);
        assert_eq!(cz, -1);
    }

    #[test]
    fn test_modified_chunk() {
        let mut mc = ModifiedChunk::new();
        let coords = IntraChunkCoords { x: 5, y: 10, z: 15 };
        let stone = BlockInstance::new(3);

        assert!(mc.get_block_at(&coords).is_none());
        mc.set_block_at(coords, stone);
        assert_eq!(mc.get_block_at(&coords), Some(&stone));
    }

    #[test]
    fn test_modified_world() {
        let mut mw = ModifiedWorld::new();
        let stone = BlockInstance::new(3);

        mw.set_block_at(5, 10, 15, stone);
        assert_eq!(mw.get_block_at(5, 10, 15), Some(&stone));
        assert!(mw.get_block_at(0, 0, 0).is_none());
    }

    #[test]
    fn test_voxel_raycast_no_hit() {
        use cgmath::{Point3, Vector3};

        let origin = Point3::new(0.0, 0.0, 0.0);
        let direction = Vector3::new(0.0, 1.0, 0.0);
        let result = voxel_raycast(&origin, &direction, 10.0, |_, _, _| false);
        assert!(result.is_none());
    }

    #[test]
    fn test_voxel_raycast_hit() {
        use cgmath::{Point3, Vector3};

        let origin = Point3::new(0.5, 0.5, 0.5);
        let direction = Vector3::new(1.0, 0.0, 0.0);
        let result = voxel_raycast(&origin, &direction, 10.0, |x, _, _| x > 2);
        assert!(result.is_some());
        let hit = result.unwrap();
        assert!(hit.block_pos.0 >= 3);
    }

    #[test]
    fn test_chunk_generation_deterministic() {
        use std::sync::{Arc, RwLock};

        let bm = Arc::new(RwLock::new(BlockManager::default()));
        let chunk1 = Chunk::generate(Arc::clone(&bm), 0, 2, 0, 42);
        let chunk2 = Chunk::generate(Arc::clone(&bm), 0, 2, 0, 42);
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
        chunk.set_block_from_xyz(0, 0, 0, BlockInstance::new(1));
        let checksum2 = chunk.compute_checksum();
        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_chunk_neighbors_from_block_pos() {
        let neighbors = Chunk::neighbors_from_block_pos(0, 0, 0);
        assert!(neighbors.contains(&(-1, 0, 0)));
        assert!(neighbors.contains(&(0, -1, 0)));
        assert!(neighbors.contains(&(0, 0, -1)));

        let inner = Chunk::neighbors_from_block_pos(16, 16, 16);
        assert!(inner.is_empty());
    }

    #[test]
    fn test_chunk_state() {
        assert_eq!(ChunkState::Pending.to_string(), "Pending");
        assert_eq!(ChunkState::Ready.to_string(), "Ready");
    }

    #[test]
    fn test_direct_normals() {
        assert_eq!(DIRECT_NORMALS_3D.len(), 6);
        assert!(DIRECT_NORMALS_3D.contains(&(0, 1, 0)));
        assert!(DIRECT_NORMALS_3D.contains(&(0, 0, -1)));
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
}
