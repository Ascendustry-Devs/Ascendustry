use crate::{
    render::meshing::{chunk::ChunkMesh, processor::GreedyMeshingProcessor},
    world::world::{MeshRequestAdd, MeshRequestDelete, MeshRequestMessage, MeshResponse, MeshSnapshot, World},
};
use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use game::constants::MAX_MESHING_CHUNKS_IN_QUEUE;
use game::world::data::chunk::ChunkState;
use project_core::{
    buffer_pool::BufferPool,
    parallel::{WorkResult, WorkerPool},
    utils::unique_queue::{FxUniqueQueue, UniqueQueue},
};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::cmp::max;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    mem,
};

pub struct WorldMesh {
    pub meshes: FxHashMap<(i32, i32, i32), ChunkMesh>,
    mesh_worker: WorkerPool<GreedyMeshingProcessor>,
    pending: FxHashMap<usize, MeshRequestAdd>,
    pending_keys: FxHashMap<(i32, i32, i32), MeshSnapshot>,
    queued: FxUniqueQueue<MeshRequestAdd>,
}

impl WorldMesh {
    pub fn new() -> WorldMesh {
        let worker_count = max(num_cpus::get() / 2, 1);
        let buffer_pool = Arc::new(BufferPool::new(1024 * 256));
        WorldMesh {
            meshes: HashMap::with_hasher(FxBuildHasher),
            mesh_worker: WorkerPool::with_max_pending(worker_count, buffer_pool, Some(MAX_MESHING_CHUNKS_IN_QUEUE as usize)),
            pending: HashMap::with_hasher(FxBuildHasher),
            pending_keys: HashMap::with_hasher(FxBuildHasher),
            queued: FxUniqueQueue::new(),
        }
    }

    pub fn init(&mut self, meshin: &mut FxHashSet<MeshRequestAdd>) {
        self.enqueue_missing_meshes(meshin);
    }

    pub fn update(&mut self, mesh_manager: &mut GpuAllocator, mesh_request: &mut MeshRequestMessage) -> Vec<MeshResponse> {
        self.enqueue_missing_meshes(&mut mesh_request.add);
        self.submit_meshes(mesh_request);
        self.compute_generated_meshes(&mut mesh_request.delete, mesh_manager)
    }

    fn enqueue_missing_meshes(&mut self, meshin: &mut FxHashSet<MeshRequestAdd>) {
        for mesh in meshin.drain() {
            self.queued.push_back(mesh);
        }
    }

    fn submit_meshes(&mut self, mesh_request: &mut MeshRequestMessage) {
        // Si la file d'attente est pleine, ça ne sert à rien d'essayer de soumettre des demandes
        if self.mesh_worker.is_queue_full() {
            return;
        }

        let mut keep_going = true;
        self.queued.retain(|chunk| {
            // Si la mesh de ce chunk doit être évincé, on ne la garde pas en file d'attente
            if mesh_request.delete.contains(&chunk.coords) {
                return false;
            }
            // Si on doit arrêter la boucle (file d'attente pleine), on garde les éléments même s'ils sont indésirables
            if !keep_going {
                return true;
            }
            // Si un traitement est déjà en cours, on attend pour cette requête
            if self.pending_keys.contains_key(&chunk.coords) {
                return true;
            }

            // On récupère les infos nécessaires pour le mesher
            let snapshot = chunk.snapshot.clone();
            let (cx, cy, cz) = chunk.coords;
            let input = (snapshot, cx, cy, cz);

            let result = self.mesh_worker.submit(input);

            match result {
                Ok(id) => {
                    // La demande a aboutit, on peut retirer la requête
                    self.pending.insert(id, chunk.clone());
                    self.pending_keys.insert(chunk.coords, chunk.snapshot.clone());
                    false
                }
                Err(_) => {
                    // La file d'attente est pleine, on arrête ici pour l'instant et on conserve cette requête
                    keep_going = true;
                    true
                }
            }
        });
    }

    fn compute_generated_meshes(
        &mut self,
        mesh_out: &mut FxHashSet<MeshRequestDelete>,
        mesh_manager: &mut GpuAllocator,
    ) -> Vec<MeshResponse> {
        let mut responses = Vec::new();

        while let Some(WorkResult { output: vertices_opt, id }) = self.mesh_worker.try_recv() {
            // Si la mesh était dans la file d'attente on la retire, sinon on passe à la suivante (déjà traitée)
            let Some(key) = self.pending.remove(&id) else {
                continue;
            };

            // On retire la mesh des clés en attente
            self.pending_keys.remove(&key.coords);

            // On récupère les données, et si elles n'existent pas, on passe à la mesh suivante
            let Some(vertices) = vertices_opt else {
                continue;
            };

            if !mesh_out.contains(&key.coords) {
                match self.mesh_at_mut(&key.coords) {
                    // Le mesh existe, on le met à jour
                    Some(mesh) => {
                        if let Some(err) = mesh.update(&vertices, mesh_manager).err() {
                            println!("Could not update mesh: {:?}", err);
                        }
                    }
                    // Le mesh n'existe pas encore, on le crée
                    None => {
                        let mut mesh = ChunkMesh::new();
                        match mesh.update(&vertices, mesh_manager) {
                            Ok(_) => {
                                // Le mesh a correctement été configuré, donc on peut l'insérer
                                self.meshes.insert(key.coords, mesh);
                            }
                            Err(e) => {
                                // Le mesh a eu un problème de configuration, on ne fait rien
                                println!("Could not insert mesh: {:?}", e as u8);
                            }
                        }
                    }
                };

                // On marque le chunk comme prêt
                responses.push(key.coords);
            }
            // Si le chunk relié à la mesh n'existe pas alors on supprime la mesh et son entrée
            else {
                if let Some(mesh) = self.meshes.remove(&key.coords) {
                    if let Some(id) = mesh.id {
                        let _ = mesh_manager.free(id);
                    }
                }
            }

            // Nettoyage
            self.mesh_worker.context().release_buffer(vertices);
        }

        responses
    }

    pub fn mesh_infos_at(&self, cpos: &(i32, i32, i32)) -> Option<(Option<u32>, bool)> {
        self.meshes.get(&cpos).map(|mesh| mesh.get_debug_infos())
    }

    pub fn mesh_at_mut(&mut self, cpos: &(i32, i32, i32)) -> Option<&mut ChunkMesh> {
        self.meshes.get_mut(&cpos)
    }

    // pub fn set_dirty(&mut self, cpos: &(i32, i32, i32)) {
    //     if let Some(chunk) = self.meshes.get_mut(&cpos) {
    //         chunk.set_dirty();
    //         self.queued.push_back(*cpos);
    //     }
    // }

    pub fn dispose(&mut self) {
        self.meshes.clear();
        self.pending.clear();
        self.pending_keys.clear();
        self.queued.clear();
        // TODO: faire fonctionner -> self.mesh_worker.dispose();
    }
}
