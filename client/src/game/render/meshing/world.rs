use crate::{
    engine::render::render::Renderer,
    game::{
        player::player::Player,
        render::meshing::chunk::{ChunkMesh, GreedyMeshingProcessor},
        world::world::World,
    },
};
use shared::parallel::{WorkResult, WorkerPool};
use shared::{buffer_pool::BufferPool, world::data::chunk::ChunkState};
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct WorldMesh {
    pub meshes: HashMap<(i32, i32, i32), ChunkMesh>,
    mesh_worker: WorkerPool<GreedyMeshingProcessor>,
    pending: HashMap<usize, (i32, i32, i32)>,
    pending_keys: HashSet<(i32, i32, i32)>,
}

impl WorldMesh {
    pub fn new() -> WorldMesh {
        let worker_count = max(num_cpus::get() / 2, 1);
        let buffer_pool = Arc::new(BufferPool::new(1024 * 256));
        WorldMesh {
            meshes: HashMap::new(),
            mesh_worker: WorkerPool::new(worker_count, buffer_pool),
            pending: HashMap::new(),
            pending_keys: HashSet::new(),
        }
    }

    pub fn mesh_at(&self, cpos: &(i32, i32, i32)) -> Option<&ChunkMesh> {
        self.meshes.get(&cpos)
    }

    pub fn mesh_at_mut(&mut self, cpos: &(i32, i32, i32)) -> Option<&mut ChunkMesh> {
        self.meshes.get_mut(&cpos)
    }

    pub fn set_dirty(&mut self, cpos: &(i32, i32, i32)) {
        if let Some(chunk) = self.meshes.get_mut(&cpos) {
            chunk.set_dirty();
        }
    }

    pub fn update(&mut self, renderer: &mut Renderer, world: &mut World, player: &Player) {
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
                    println!("yes");
                    let snapshot = world.get_mesh_snapshot(cx, cy, cz);
                    if let Ok(id) = self.mesh_worker.submit((Arc::clone(&chunk_data.chunk), snapshot, cx, cy, cz)) {
                        self.pending.insert(id, key);
                        self.pending_keys.insert(key);
                    }
                }
            }
        }
        // println!("Mesh infos: {} {}", self.meshes.len(), self.meshes.capacity(),);

        while let Some(WorkResult { output: vertices_opt, id }) = self.mesh_worker.try_recv() {
            if let Some(key) = self.pending.remove(&id) {
                self.pending_keys.remove(&key);

                let Some(vertices) = vertices_opt else {
                    continue;
                };

                if let Some(chunk) = world.get_chunk_data_mut(key.0, key.1, key.2) {
                    match self.mesh_at_mut(&key) {
                        Some(mesh) => match mesh.update(&vertices, renderer) {
                            Ok(_) => {}
                            Err(e) => {
                                println!("Could not update mesh: {:?}", e as u8);
                            }
                        },
                        None => {
                            let mut mesh = ChunkMesh::new();
                            match mesh.update(&vertices, renderer) {
                                Ok(_) => {
                                    self.meshes.insert(key, mesh);
                                }
                                Err(e) => {
                                    println!("Could not insert mesh: {:?}", e as u8);
                                }
                            }
                        }
                    };
                    chunk.is_dirty = false;
                    chunk.state = ChunkState::Ready;
                } else {
                    self.meshes.remove(&key);
                }

                self.mesh_worker.context().release_buffer(vertices);
            }
        }
    }

    pub fn dispose(&mut self) {
        self.meshes.clear();
        self.pending.clear();
        self.pending_keys.clear();
        // TODO: faire fonctionner -> self.mesh_worker.dispose();
    }
}
