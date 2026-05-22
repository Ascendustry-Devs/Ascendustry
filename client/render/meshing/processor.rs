use std::sync::Arc;

use satiscore::{
    buffer_pool::BufferPool,
    geometry::vertex::Vertex,
    parallel::Parallelizable,
    world::data::chunk::{Chunk, CHUNK_SIZE},
};

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
    type Input = (Arc<Chunk>, MeshSnapshot, i32, i32, i32);
    type Output = Option<Vec<Vertex>>;

    /// Makes greedy
    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let (main_chunk, neighbors, cx, cy, cz) = input;

        let padded = PaddedChunk::from_snapshot(&main_chunk, &neighbors);

        // Pre-calc entire chunk blocks solidity to save CPU (by avoiding repetition)
        let mut solidity = [false; PADDED_CHUNK_BLOCK_CBE_USIZE];

        for i in 0..PADDED_CHUNK_BLOCK_CBE_USIZE {
            solidity[i] = padded.get_block_from_i(i).is_solid();
        }

        // Pre-calc chunk world position to save CPU (by avoiding repetition)
        let (cwx, cwy, cwz) = ((cx * CHUNK_SIZE) as f32, (cy * CHUNK_SIZE) as f32, (cz * CHUNK_SIZE) as f32);

        let mut vertices = ctx.get_buffer();

        ChunkMesh::make_greedy_x(&padded, &solidity, &mut vertices, cwx, cwy, cwz);
        ChunkMesh::make_greedy_y(&padded, &solidity, &mut vertices, cwx, cwy, cwz);
        ChunkMesh::make_greedy_z(&padded, &solidity, &mut vertices, cwx, cwy, cwz);

        return Some(vertices);
    }
}
