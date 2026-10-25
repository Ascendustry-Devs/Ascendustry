use crate::{
    render::meshing::{chunk::ChunkMesh, processor::GreedyMeshingProcessor},
    world::mesh_request::{
        MeshRequestAdd, MeshRequestDelete, MeshRequestMessage, MeshRequestQueue, MeshResponse, MeshSnapshot,
    },
};
use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use game::constants::MAX_MESHING_CHUNKS_IN_QUEUE;
use project_core::{
    buffer_pool::BufferPool,
    parallel::{WorkResult, WorkerPool},
};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::{cmp::max, mem::take, sync::RwLock};

pub struct WorldMesh {
    pub meshes: FxHashMap<(i32, i32, i32), ChunkMesh>,
    pub chunk_meshes: FxHashSet<(i32, i32, i32)>,
    mesh_worker: WorkerPool<GreedyMeshingProcessor>,
    pending: FxHashMap<usize, MeshRequestAdd>,
    pending_keys: FxHashMap<(i32, i32, i32), MeshSnapshot>,
    /// Requêtes de maillage en attente de soumission, ordonnées par priorité.
    queued: MeshRequestQueue,
    /// Position du joueur (en coordonnées de chunk), utilisée pour trier la
    /// file par priorité. `None` tant que la position n'est pas connue.
    player_chunk: Option<(i32, i32, i32)>,

    alloc: Option<Arc<RwLock<GpuAllocator>>>,
    texture_lookup: Arc<Vec<u32>>,
}

impl Default for WorldMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldMesh {
    pub fn new() -> Self {
        let worker_count = max(num_cpus::get() / 2, 1);
        // Le pool est borné : sinon chaque buffer rendu (256 K sommets) reste
        // en mémoire indéfiniment et l'empreinte explose avec le nombre de chunks.
        let buffer_pool = Arc::new(BufferPool::with_max_buffers(1024 * 256, worker_count));
        Self {
            meshes: HashMap::with_hasher(FxBuildHasher),
            chunk_meshes: HashSet::with_hasher(FxBuildHasher),
            mesh_worker: WorkerPool::with_max_pending(worker_count, buffer_pool, Some(MAX_MESHING_CHUNKS_IN_QUEUE as usize)),
            pending: HashMap::with_hasher(FxBuildHasher),
            pending_keys: HashMap::with_hasher(FxBuildHasher),
            queued: MeshRequestQueue::with_capacity(MAX_MESHING_CHUNKS_IN_QUEUE as usize),
            player_chunk: None,
            alloc: None,
            texture_lookup: Arc::new(Vec::new()),
        }
    }

    pub fn init(&mut self, alloc: Arc<RwLock<GpuAllocator>>, mesh_request: &mut MeshRequestMessage) {
        self.alloc = Some(alloc);
        self.listen(mesh_request);
    }

    pub fn set_texture_lookup(&mut self, texture_lookup: Arc<Vec<u32>>) {
        self.texture_lookup = texture_lookup;
    }

    /// Indique la position du joueur (en coordonnées de chunk), utilisée pour
    /// trier les requêtes de maillage par priorité.
    pub fn set_player_chunk(&mut self, player_chunk: (i32, i32, i32)) {
        self.player_chunk = Some(player_chunk);
    }

    pub fn update(
        &mut self,
        mesh_manager: &mut Arc<RwLock<GpuAllocator>>,
        mesh_request: &mut MeshRequestMessage,
    ) -> Vec<MeshResponse> {
        self.listen(mesh_request);
        self.submit_meshes();
        self.compute_generated_meshes(&mesh_request.delete, mesh_manager)
    }

    fn listen(&mut self, mesh_request: &mut MeshRequestMessage) {
        let input = &mut mesh_request.add;
        let output = &mut mesh_request.delete;
        for request in input.drain() {
            self.add_to_mesh(request);
        }
        for mesh in output.drain() {
            self.delete_mesh(&mesh);
        }
    }

    fn add_to_mesh(&mut self, request: MeshRequestAdd) {
        self.chunk_meshes.insert(request.coords);
        // `insert` remplace l'instantané si le chunk était déjà en attente,
        // tout en conservant sa position dans la file : une modification de
        // bloc sur un chunk pas encore maillé n'est donc jamais perdue.
        self.queued.insert(request);
    }

    fn delete_mesh_with_alloc(&mut self, alloc: &mut GpuAllocator, coords: &(i32, i32, i32)) {
        self.chunk_meshes.remove(coords);
        self.pending.retain(|_, v| v.coords.ne(coords));
        self.pending_keys.remove(coords);
        if let Some(mesh) = self.meshes.remove(coords) {
            if let Some(mesh_id) = mesh.id {
                match alloc.free(mesh_id) {
                    Ok(_) => {}
                    Err(e) => {
                        println!(
                            "WorldMesh delete_mesh: could not free mesh {:?} with id {}.\nError: {}",
                            coords, mesh_id, e
                        );
                    }
                }
            }
        }
    }

    fn delete_mesh(&mut self, coords: &(i32, i32, i32)) {
        let raw_alloc = take(&mut self.alloc).unwrap();
        {
            let alloc = &mut raw_alloc.write().unwrap();
            self.delete_mesh_with_alloc(alloc, coords);
        }
        self.alloc = Some(raw_alloc);
    }

    fn submit_meshes(&mut self) {
        // Si la file d'attente est pleine, ça ne sert à rien d'essayer de soumettre des demandes
        if self.mesh_worker.is_queue_full() {
            return;
        }

        // Les chunks sont maillés **du plus proche du joueur au plus loin** :
        // c'est ce qui évite les "trous" visuels autour du joueur et rend
        // l'apparition du monde progressive et radiale.
        if let Some(player_chunk) = self.player_chunk {
            self.queued.sort_by_priority(player_chunk);
        }

        // Drapeau permettant d'interrompre le parcours dès que la file du
        // pool de maillage est saturée.
        let mut queue_saturated = false;

        self.queued.retain(|chunk| {
            // La file du pool est pleine : on s'arrête et on conserve le reste
            // de la file (qui sera servie lors d'un prochain appel).
            if queue_saturated {
                return true;
            }
            // Si ce chunk n'est pas enregistré, on ne le traite pas
            if !self.chunk_meshes.contains(&chunk.coords) {
                return false;
            }
            // Si un traitement est déjà en cours, on attend pour cette requête
            if self.pending_keys.contains_key(&chunk.coords) {
                return true;
            }

            // On récupère les infos nécessaires pour le mesher
            let snapshot = chunk.snapshot.clone();
            let (cx, cy, cz) = chunk.coords;
            let input = (snapshot, cx, cy, cz, self.texture_lookup.clone());

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
                    queue_saturated = true;
                    true
                }
            }
        });
    }

    fn compute_generated_meshes(
        &mut self,
        mesh_out: &FxHashSet<MeshRequestDelete>,
        mesh_manager: &Arc<RwLock<GpuAllocator>>,
    ) -> Vec<MeshResponse> {
        let mut responses = Vec::new();

        let alloc = &mut mesh_manager.write().unwrap();
        while let Some(WorkResult {
            output: vertices_opt,
            id,
        }) = self.mesh_worker.try_recv()
        {
            // Si la mesh était dans la file d'attente on la retire, sinon on passe à la suivante (déjà traitée)
            let Some(key) = self.pending.remove(&id) else {
                // Nettoyage
                if let Some(vertices) = vertices_opt {
                    self.mesh_worker.context().release_buffer(vertices);
                }
                continue;
            };

            // On retire la mesh des clés en attente
            self.pending_keys.remove(&key.coords);

            // On récupère les données, et si elles n'existent pas, on passe à la mesh suivante
            let Some(vertices) = vertices_opt else {
                // Nettoyage
                self.delete_mesh_with_alloc(alloc, &key.coords);
                if let Some(vertices) = vertices_opt {
                    self.mesh_worker.context().release_buffer(vertices);
                }
                continue;
            };

            // Le mesh n'est PAS à supprimer
            if !mesh_out.contains(&key.coords) && self.chunk_meshes.contains(&key.coords) {
                match self.mesh_at_mut(&key.coords) {
                    // Le mesh existe, on le met à jour
                    Some(mesh) => {
                        if let Some(err) = mesh.update(&vertices, alloc).err() {
                            println!("Could not update mesh: {:?}", err);
                        }
                        // Le mesh GPU a été libéré car ses données sont à présent vides.
                        // On libère également le mesh CPU.
                        if mesh.id.is_none() {
                            self.meshes.remove(&key.coords);
                        }
                    }
                    // Le mesh n'existe pas encore, on le crée
                    None => {
                        let mut mesh = ChunkMesh::new();
                        match mesh.update(&vertices, alloc) {
                            Ok(_) => {
                                // Le mesh a correctement été configuré, donc on peut l'insérer
                                if mesh.id.is_some() {
                                    self.meshes.insert(key.coords, mesh);
                                } else {
                                    // Si le mesh est vide, il est normal qu'on ne l'insère pas (l'id n'a pas été attribué donc on arrive ici).
                                    // En revanche, s'il contient des données, on l'affiche dans la console.
                                    if !vertices.is_empty() {
                                        println!(
                                            "WorldMesh compute_generated_meshes: Could not insert mesh, for unknown reason."
                                        );
                                    }
                                    self.mesh_worker.context().release_buffer(vertices);
                                    continue;
                                }
                            }
                            Err(e) => {
                                // Le mesh a eu un problème, on le retire
                                println!(
                                    "WorldMesh compute_generated_meshes: Could not update to-insert-mesh.\nError: {:?}",
                                    e
                                );
                                // Nettoyage
                                self.mesh_worker.context().release_buffer(vertices);
                                continue;
                            }
                        }
                    }
                };

                // On marque le chunk comme prêt
                responses.push(key.coords);
            }
            // Le mesh doit être supprimé.
            else {
                self.delete_mesh_with_alloc(alloc, &key.coords);
            }

            // Nettoyage
            self.mesh_worker.context().release_buffer(vertices);
        }

        responses
    }

    pub fn mesh_infos_at(&self, cpos: &(i32, i32, i32)) -> Option<(Option<u32>, bool)> {
        self.meshes.get(cpos).map(|mesh| mesh.get_debug_infos())
    }

    pub fn mesh_at_mut(&mut self, cpos: &(i32, i32, i32)) -> Option<&mut ChunkMesh> {
        self.meshes.get_mut(cpos)
    }

    pub fn print_memory(&self) {
        let conversion = |b: u32| {
            let kb = b / 1024;
            (kb, b)
        };

        // Does not include mesh_worker buffers
        let alloc = (self.chunk_meshes.capacity() * size_of::<(i32, i32, i32)>()
                + self.meshes.capacity() * size_of::<((i32, i32, i32), ChunkMesh)>()
                + self.pending.capacity() * size_of::<(usize, MeshRequestAdd)>()
                + self.pending_keys.capacity() * size_of::<((i32, i32, i32), MeshSnapshot)>()
                // La file est composée d'un Vec de coordonnées et d'une table
                // de hachage des requêtes.
                + self.queued.capacity() * size_of::<(i32, i32, i32)>()
                + self.queued.requests_capacity()
                    * (size_of::<(i32, i32, i32)>() + size_of::<MeshRequestAdd>())) as u32;

        let used = (self.chunk_meshes.len() * size_of::<(i32, i32, i32)>()
            + self.meshes.len() * size_of::<((i32, i32, i32), ChunkMesh)>()
            + self.pending.len() * size_of::<(usize, MeshRequestAdd)>()
            + self.pending_keys.len() * size_of::<((i32, i32, i32), MeshSnapshot)>()
            + self.queued.len() * size_of::<(i32, i32, i32)>()
            + self.queued.len() * (size_of::<(i32, i32, i32)>() + size_of::<MeshRequestAdd>())) as u32;

        let (alloc_kb, alloc_b) = conversion(alloc);
        let (used_kb, used_b) = conversion(used);
        let (free_kb, free_b) = (alloc_kb - used_kb, alloc_b - used_b);

        println!(
            "Mesh count/capacity: {}/{} ({:3.1}%)",
            self.meshes.len(),
            self.meshes.capacity(),
            (100f32 * self.meshes.len() as f32) / (self.meshes.capacity() as f32)
        );
        println!("Allocated Memory (CPU) {:3}Kb | {:6}b", alloc_kb, alloc_b);
        println!("└─ Free                {:3}Kb | {:6}b", free_kb, free_b);
        println!("└─ Used                {:3}Kb | {:6}b", used_kb, used_b);
    }

    pub fn dispose(&mut self, mesh_manager: &mut Arc<RwLock<GpuAllocator>>) {
        let mut alloc = mesh_manager.write().unwrap();
        for mesh in self.meshes.drain() {
            if let Some(mesh_id) = mesh.1.id {
                let _ = alloc.free(mesh_id);
            }
        }
        self.pending.clear();
        self.pending_keys.clear();
        self.queued.clear();
        // TODO: faire fonctionner -> self.mesh_worker.dispose();
    }
}
