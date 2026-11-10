use std::sync::Arc;

use engine::geometry::vertex::Vertex;
use game::{types::Position, world::chunk_position::ChunkPosition};
use project_core::parallel::{buffer_pool::BufferPool, Parallelizable};

use crate::{
    render::meshing::chunk_new::make_greedy_chunk, systems::texture_registry::TextureRegistry,
    world::mesh_request::MeshSnapshot,
};

pub struct GreedyMeshingProcessor;

impl Parallelizable for GreedyMeshingProcessor {
    type Context = Arc<BufferPool<Vertex>>;
    type Input = (MeshSnapshot, ChunkPosition, Arc<TextureRegistry>);
    type Output = Option<Vec<Vertex>>;

    /// Makes greedy
    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let (snapshot, position, textures) = input;

        // Origine monde du chunk (indispensable : sinon tous les chunks se superposent à l'origine).
        let origin: Position = (&position).into();

        let mut vertices = ctx.get_buffer();

        make_greedy_chunk(&snapshot, &mut vertices, [*origin.x(), *origin.y(), *origin.z()], textures);

        Some(vertices)
    }
}
