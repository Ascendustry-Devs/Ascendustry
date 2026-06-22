use std::sync::Arc;

use engine::geometry::vertex::Vertex;
use game::world::data::chunk::CHUNK_SIZE;
use project_core::{buffer_pool::BufferPool, parallel::Parallelizable};

use crate::{
    render::{
        meshing::chunk::ChunkMesh,
        utils::padded_chunk::{PaddedChunk, PADDED_CHUNK_BLOCK_CBE_USIZE},
    },
    world::world::MeshSnapshot,
};

pub struct GreedyMeshingProcessor;

impl Parallelizable for GreedyMeshingProcessor {
    type Context = Arc<BufferPool<Vertex>>;
    type Input = (MeshSnapshot, i32, i32, i32, Arc<Vec<u32>>);
    type Output = Option<Vec<Vertex>>;

    /// Makes greedy
    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let (snapshot, cx, cy, cz, texture_lookup) = input;

        let padded = PaddedChunk::from_snapshot(&snapshot.main, &snapshot);

        // Pre-calc entire chunk blocks solidity to save CPU (by avoiding repetition)
        let mut solidity = [false; PADDED_CHUNK_BLOCK_CBE_USIZE];

        for i in 0..PADDED_CHUNK_BLOCK_CBE_USIZE {
            solidity[i] = padded.get_block_from_i(i).is_solid();
        }

        // Pre-calc chunk world position to save CPU (by avoiding repetition)
        let (cwx, cwy, cwz) = ((cx * CHUNK_SIZE) as f32, (cy * CHUNK_SIZE) as f32, (cz * CHUNK_SIZE) as f32);

        let mut vertices = ctx.get_buffer();

        ChunkMesh::make_greedy_x(&padded, &solidity, &mut vertices, cwx, cwy, cwz, &texture_lookup);
        ChunkMesh::make_greedy_y(&padded, &solidity, &mut vertices, cwx, cwy, cwz, &texture_lookup);
        ChunkMesh::make_greedy_z(&padded, &solidity, &mut vertices, cwx, cwy, cwz, &texture_lookup);

        return Some(vertices);
    }
}
