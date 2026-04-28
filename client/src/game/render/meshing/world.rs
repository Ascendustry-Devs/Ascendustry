use crate::{
    engine::render::render::Renderer,
    game::{
        player::player::Player,
        render::meshing::chunk::{ChunkMesh, GreedyMeshingProcessor},
        world::world::World,
    },
};
use shared::parallel::{WorkResult, WorkerPool};
use std::collections::{HashMap, HashSet};

pub struct WorldMesh {
    pub meshes: HashMap<(i32, i32, i32), ChunkMesh>,
    mesh_worker: WorkerPool<GreedyMeshingProcessor>,
    pending: HashMap<usize, (i32, i32, i32)>,
    pending_keys: HashSet<(i32, i32, i32)>,
}

impl WorldMesh {
    pub fn new() -> WorldMesh {
        let worker_count = num_cpus::get();
        WorldMesh {
            meshes: HashMap::new(),
            mesh_worker: WorkerPool::new(worker_count, ()),
            pending: HashMap::new(),
            pending_keys: HashSet::new(),
        }
    }

    pub fn update(&mut self, renderer: &mut Renderer, world: &World, player: &Player) {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = player.get_rendered_chunk_range();
        let mut needed_rendered_keys: Vec<(i32, i32, i32)> = Vec::new();

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    needed_rendered_keys.push((x, y, z));
                }
            }
        }

        for &(cx, cy, cz) in needed_rendered_keys.iter() {
            let key = (cx, cy, cz);

            if self.pending_keys.contains(&key) {
                continue;
            }

            if let Some(chunk_data) = world.get_chunk_data(cx, cy, cz) {
                let needs_processing = self.meshes.get(&key).map_or(true, |mesh: &ChunkMesh| mesh.is_dirty());

                if needs_processing && world.are_all_neighbors_ready(cx, cy, cz) {
                    let snapshot = world.get_mesh_snapshot(cx, cy, cz);
                    if let Ok(id) = self.mesh_worker.submit((chunk_data.chunk.clone().into(), snapshot, cx, cy, cz)) {
                        self.pending.insert(id, key);
                        self.pending_keys.insert(key);
                    }
                }
            }
        }

        while let Some(WorkResult { output: vertices_opt, id }) = self.mesh_worker.try_recv() {
            if let Some(key) = self.pending.remove(&id) {
                self.pending_keys.remove(&key);
                self.meshes.remove(&key);

                if let Some(vertices) = vertices_opt {
                    let mut mesh = ChunkMesh::new();
                    mesh.update(vertices, renderer);
                    self.meshes.insert(key, mesh);
                }
            }
        }
    }
}
