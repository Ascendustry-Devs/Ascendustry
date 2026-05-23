use rustc_hash::{FxBuildHasher, FxHashMap};
use satiscore::{
    buffer_pool::BufferPool,
    utils::unique_queue::{FxUniqueQueue, UniqueQueue},
    world::data::chunk::ChunkState,
};
use satiscore::{
    constants::MAX_MESHING_CHUNKS_IN_QUEUE,
    parallel::{WorkResult, WorkerPool},
};
use std::cmp::max;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    mem,
};
use {
    crate::{
        render::meshing::{chunk::ChunkMesh, processor::GreedyMeshingProcessor},
        world::world::World,
    },
    engine::render::mesh::manager::MeshManager,
};

pub struct WorldMesh {
    pub meshes: FxHashMap<(i32, i32, i32), ChunkMesh>,
    mesh_worker: WorkerPool<GreedyMeshingProcessor>,
    pending: HashMap<usize, (i32, i32, i32)>,
    pending_keys: HashSet<(i32, i32, i32)>,
    queued: FxUniqueQueue<(i32, i32, i32)>,
}

impl WorldMesh {
    pub fn new() -> WorldMesh {
        let worker_count = max(num_cpus::get() / 2, 1);
        let buffer_pool = Arc::new(BufferPool::new(1024 * 256));
        WorldMesh {
            meshes: HashMap::with_hasher(FxBuildHasher),
            mesh_worker: WorkerPool::with_max_pending(worker_count, buffer_pool, Some(MAX_MESHING_CHUNKS_IN_QUEUE as usize)),
            pending: HashMap::new(),
            pending_keys: HashSet::new(),
            queued: FxUniqueQueue::new(),
        }
    }

    pub fn init(&mut self, world: &mut World) {
        self.enqueue_missing_meshes(world);
        self.submit_meshes(world);
    }

    pub fn update(&mut self, mesh_manager: &mut MeshManager, world: &mut World) {
        // self.clean_meshes();
        self.enqueue_missing_meshes(world);
        self.submit_meshes(world);
        self.compute_generated_meshes(mesh_manager, world);
    }

    // fn clean_meshes(&mut self) {
    //     self.meshes.iter()
    // }

    fn enqueue_missing_meshes(&mut self, world: &mut World) {
        // Récupérer les chunks prêts à être meshés
        let ready = world.ready_to_mesh();
        let mut ready = mem::replace(ready, UniqueQueue::new());

        ready.retain(|chunk| {
            let (cx, cy, cz) = *chunk;

            // Si le chunk n'existe pas, on ne crée pas le mesh
            if world.get_chunk_data(cx, cy, cz).is_none() {
                return true;
            };

            // Si les chunks voisins sont prêts, on le met en file d'attente pour le mesh
            if world.are_all_neighbors_ready(cx, cy, cz) {
                self.queued.push_back(*chunk);
                false
            }
            // Si les chunks voisins ne sont pas encore générés, on le laisse en attente
            else {
                true
            }
        });
    }

    fn submit_meshes(&mut self, world: &mut World) {
        // Si la file d'attente est pleine, ça ne sert à rien d'essayer de soumettre des demandes
        if self.mesh_worker.is_queue_full() {
            return;
        }

        let mut keep_going = true;
        self.queued.retain(|chunk| {
            // Si on doit arrêter la boucle (file d'attente pleine), on garde les éléments même s'ils sont indésirables
            if !keep_going {
                return true;
            }
            // Si un traitement est déjà en cours, on attend pour cette requête
            if self.pending_keys.contains(chunk) {
                return true;
            }

            let (cx, cy, cz) = *chunk;

            // Si le chunk associé au mesh n'existe pas, on retire la requête
            let Some(chunk_data) = world.get_chunk_data(cx, cy, cz) else {
                return false;
            };

            // On récupère les infos nécessaires pour le mesher
            let chunk_copy = Arc::clone(&chunk_data.chunk);
            let snapshot = world.get_mesh_snapshot(cx, cy, cz);
            let input = (chunk_copy, snapshot, cx, cy, cz);

            let result = self.mesh_worker.submit(input);

            match result {
                Ok(id) => {
                    // La demande a aboutit, on peut retirer la requête
                    self.pending.insert(id, *chunk);
                    self.pending_keys.insert(*chunk);
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

    fn compute_generated_meshes(&mut self, mesh_manager: &mut MeshManager, world: &mut World) {
        while let Some(WorkResult { output: vertices_opt, id }) = self.mesh_worker.try_recv() {
            // Si la mesh était dans la file d'attente on la retire, sinon on passe à la suivante (déjà traitée)
            let Some(key) = self.pending.remove(&id) else {
                continue;
            };

            // On retire la mesh des clés en attente
            self.pending_keys.remove(&key);

            // On récupère les données, et si elles n'existent pas, on passe à la mesh suivante
            let Some(vertices) = vertices_opt else {
                continue;
            };

            if let Some(chunk) = world.get_chunk_data_mut(key.0, key.1, key.2) {
                match self.mesh_at_mut(&key) {
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
                                self.meshes.insert(key, mesh);
                            }
                            Err(e) => {
                                // Le mesh a eu un problème de configuration, on ne fait rien
                                println!("Could not insert mesh: {:?}", e as u8);
                            }
                        }
                    }
                };

                // On marque le chunk comme prêt
                chunk.state = ChunkState::Ready;
            }
            // Si le chunk relié à la mesh n'existe pas alors on supprime la mesh et son entrée
            else {
                self.meshes.remove(&key);
            }

            // Nettoyage
            self.mesh_worker.context().release_buffer(vertices);
        }
    }

    pub fn mesh_infos_at(&self, cpos: &(i32, i32, i32)) -> Option<(Option<u32>, bool)> {
        self.meshes.get(&cpos).map(|mesh| mesh.get_debug_infos())
    }

    pub fn mesh_at_mut(&mut self, cpos: &(i32, i32, i32)) -> Option<&mut ChunkMesh> {
        self.meshes.get_mut(&cpos)
    }

    pub fn set_dirty(&mut self, cpos: &(i32, i32, i32)) {
        if let Some(chunk) = self.meshes.get_mut(&cpos) {
            chunk.set_dirty();
            self.queued.push_back(*cpos);
        }
    }

    pub fn dispose(&mut self) {
        self.meshes.clear();
        self.pending.clear();
        self.pending_keys.clear();
        self.queued.clear();
        // TODO: faire fonctionner -> self.mesh_worker.dispose();
    }
}
