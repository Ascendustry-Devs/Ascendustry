pub mod mesh_request;

use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use engine::render::modes::RenderMode;
use game::assets::block_loader::load_block_definitions;
use game::assets::item_loader::load_item_definitions;
use game::assets::resolve_asset_path;
use game::constants::{
    DIRECT_NORMALS_3D, HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F, MAX_GENERATION_CHUNKS_IN_QUEUE,
    SIMULATION_DISTANCE_CHUNK_COUNT,
};
use game::inventory::item_manager::ItemId;
use game::inventory::{ItemInstance, ItemManager};
use game::world::chunk_position::ChunkPosition;
use game::world::data::block::{BlockId, BlockInstance, BlockManager};
use game::world::data::chunk::{Chunk, ChunkData, ChunkState, CHUNK_BLOCK_NUMBER, CHUNK_SIZE_HALFED_VEC3_F};
use game::world::generation::chunk_generator::ChunkGenerator;
use game::world::local_position::LocalPosition;
use game::world::scheduling::compute_missing_chunks_sorted;
use game::world::world_position::WorldPosition;
use game::world::{ChunkModification, ChunkStorage, World};
// use engine::gpu::allocator::gpu_allocator::GpuAllocator;
// use engine::render::modes::RenderMode;
use num_cpus::get;
use physics::aabb::AABB;
use physics::collision_world::CollisionWorld;
use project_core::{log_client, log_err_client, log_warn_server};

// use crate::api::texture_loader::TextureLoader;
use project_core::general_types::unique_queue::{FxUniqueQueue, UniqueQueue};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::cmp::min;
use std::collections::hash_map::Keys;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::{cmp::max, collections::VecDeque, mem};

use crate::api::texture_loader::TextureLoader;
use crate::{
    player::Player,
    render::meshing::world::WorldMesh,
    world::mesh_request::{MeshRequestAdd, MeshRequestMessage, MeshResponse, MeshSnapshot},
};

pub struct ClientWorld {
    seed: u32,
    chunks: ClientChunks,
    chunk_generator: ChunkGenerator,
    block_manager: Arc<RwLock<BlockManager>>,
    item_manager: Arc<RwLock<ItemManager>>,
    /// Chunks déjà générés mais pas encore maillés (il manque des voisins).
    waiting_to_mesh: FxUniqueQueue<ChunkPosition>,
    /// Chunks restant à générer, triés par priorité (distance au joueur).
    ///
    /// Cette liste est **persistante** : les chunks qui n'ont pas pu être
    /// demandés au générateur (file pleine) y restent et seront servis dès
    /// que de la place se libère, au lieu d'être perdus.
    missing_chunks: VecDeque<ChunkPosition>,
    /// Chunks demandés au générateur mais pas encore reçus.
    ///
    /// Sert à ne pas demander deux fois le même chunk (le générateur étant
    /// asynchrone, un chunk en cours de génération n'est pas encore présent
    /// dans `chunks`).
    in_flight: FxHashSet<ChunkPosition>,
    mesh_request: MeshRequestMessage,
}

impl ClientWorld {
    pub fn new(seed: u32) -> Self {
        let block_manager = Arc::new(RwLock::new(BlockManager::new()));
        let item_manager = Arc::new(RwLock::new(ItemManager::new()));

        let cpus_count = get();
        let worker_count = max(cpus_count / 2, min(cpus_count, 2));
        println!("Available cpus: {}", cpus_count);
        println!("ChunkGenerator worker_count: {}", worker_count);
        let chunk_generator = ChunkGenerator::with_max_pending(
            worker_count,
            Arc::clone(&block_manager),
            seed,
            MAX_GENERATION_CHUNKS_IN_QUEUE as usize,
        );

        Self {
            seed,
            chunks: ClientChunks::new(),
            chunk_generator,
            block_manager,
            item_manager,
            waiting_to_mesh: UniqueQueue::with_capacity(256),
            missing_chunks: VecDeque::with_capacity(SIMULATION_DISTANCE_CHUNK_COUNT as usize),
            in_flight: HashSet::with_hasher(FxBuildHasher),
            mesh_request: MeshRequestMessage::empty(),
        }
    }

    pub fn init(&mut self, texture_loader: &mut TextureLoader, player: &Player) -> &mut MeshRequestMessage {
        {
            let mut item_manager = self.item_manager.write().unwrap();
            let items = load_item_definitions(resolve_asset_path("items/")).expect("Failed to load item definitions");
            for item in items {
                let id = item_manager.register(item.clone());
                if let Some(tex_path) = &item.texture_path {
                    match texture_loader.register_item(&id, tex_path.clone(), RenderMode::Opaque) {
                        Ok(texture_id) => log_client!(
                            "Successfully added item {} to the texture registry with id {:?}.",
                            item.name,
                            texture_id
                        ),
                        Err(err) => {
                            log_err_client!("Failed to add item {} to the texture registry. Error: {:?}", item.name, err)
                        }
                    }
                }
            }
        }

        {
            let mut block_manager = self.block_manager.write().unwrap();
            let item_manager = self.item_manager.read().unwrap();

            let defs = load_block_definitions(resolve_asset_path("blocks/")).expect("Failed to load block definitions");

            for def in defs {
                let id = block_manager.register(def.clone());
                if let Some(tex_path) = &def.texture_path {
                    let render_mode = match def.render_mode.to_lowercase().as_str() {
                        "opaque" => RenderMode::Opaque,
                        _ => RenderMode::Opaque,
                    };
                    match texture_loader.register_block(&id, tex_path.clone(), render_mode) {
                        Ok(texture_id) => log_client!(
                            "Successfully added block {} to the texture registry with id {:?}.",
                            def.name,
                            texture_id
                        ),
                        Err(err) => {
                            log_err_client!("Failed to add block {} to the texture registry. Error: {:?}", def.name, err)
                        }
                    }
                }
            }

            block_manager.resolve_item_mappings(&item_manager);
        }

        self.refresh_missing_chunks(player);
        &mut self.mesh_request
    }

    pub fn update(
        &mut self,
        mesh_manager: &mut Arc<RwLock<GpuAllocator>>,
        world_mesh: &mut WorldMesh,
        player: &Player,
    ) -> &mut MeshRequestMessage {
        if player.state.cpos.has_changed() || self.chunks.is_empty() {
            self.clean_chunks(mesh_manager, world_mesh, player);
            // La liste des chunks manquants est reconstruite et re-triée par
            // rapport à la nouvelle position du joueur.
            self.refresh_missing_chunks(player);
        }
        // Récupération des chunks générés par le pool.
        self.compute_generated_chunks();
        // Remplissage du générateur : autant que sa file le permet, et non
        // seulement quand le joueur bouge.
        self.pump_generation();
        self.submit_to_mesh();
        &mut self.mesh_request
    }

    pub fn listen(&mut self, mesh_responses: &mut Vec<MeshResponse>) {
        for response in mesh_responses.drain(..) {
            if let Some(chunk) = self.chunks.get_mut(&response) {
                chunk.state = ChunkState::Ready;
            }
        }
    }

    pub fn chunk_infos_at(&self, position: &ChunkPosition) -> Option<(ChunkState, bool)> {
        self.chunks.get(position).map(|chunk| chunk.get_debug_infos())
    }

    pub fn block_to_item(&self, block_id: &BlockId) -> Option<ItemInstance> {
        let bm = self.block_manager.read().unwrap();
        let im = self.item_manager.read().unwrap();
        bm.block_to_item(block_id, &im)
    }

    pub fn item_to_block(&self, item_id: &ItemId) -> Option<BlockId> {
        self.block_manager.read().unwrap().item_to_block_id(item_id)
    }

    fn clean_chunks(&mut self, mesh_manager: &Arc<RwLock<GpuAllocator>>, world_mesh: &mut WorldMesh, player: &Player) {
        let alloc = &mut mesh_manager.write().unwrap();

        // Coordonnées du chunk en blocks où se trouve le joueur
        let chunk_pos_in_blocks = player.get_cpos().into();
        let player_simulation_aabb = AABB::new_sized(chunk_pos_in_blocks, HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F);

        self.chunks.retain(|key, _| {
            let key_vec_f = key.into();
            let chunk_aabb = AABB::new_sized(key_vec_f, CHUNK_SIZE_HALFED_VEC3_F);

            // On garde les chunks qui sont à portée du joueur
            if chunk_aabb.overlaps(&player_simulation_aabb) {
                return true;
            }

            // On supprime proprement ceux qui ne le sont plus
            self.mesh_request.delete.insert(*key);
            let Some(mesh) = world_mesh.meshes.remove(key) else {
                return false;
            };
            let Some(mesh_id) = mesh.id else {
                return false;
            };
            let result = alloc.free(mesh_id);
            match result {
                Ok(_) => {}
                Err(err) => {
                    project_core::log_err_client!(
                        "World: an error occured while trying to free chunk (pos: {:?})'s mesh with id {}.\nError: {}",
                        *key,
                        mesh_id,
                        err
                    )
                }
            }
            false
        });
    }

    /// Recalcule la liste des chunks restant à générer, triés par priorité
    /// (distance au joueur).
    ///
    /// Sont exclus :
    /// - les chunks déjà générés (`chunks`) ;
    /// - les chunks déjà demandés au générateur et pas encore reçus
    ///   (`in_flight`) : les redemander produirait du travail inutile.
    ///
    /// Appelé lorsque le joueur change de chunk : la liste est reconstruite
    /// depuis zéro, ce qui a pour effet de **re-prioriser** les chunks encore
    /// manquants par rapport à la nouvelle position.
    fn refresh_missing_chunks(&mut self, player: &Player) {
        let player_chunk = player.get_cpos();

        let missing = compute_missing_chunks_sorted(
            &mut player.get_simulation_chunk_keys().iter(),
            &self.chunks.keys().copied().collect(),
            &self.in_flight,
            &player_chunk,
        );

        self.missing_chunks = missing.into();
    }

    /// Demande au générateur autant de chunks que sa file peut en accepter.
    ///
    /// Les chunks sont pris dans l'ordre de priorité. Ceux qui n'ont pas pu
    /// être demandés restent dans la file (`missing_chunks`) et seront servis
    /// dès que de la place se libère.
    fn pump_generation(&mut self) {
        while !self.chunk_generator.is_queue_full() {
            let Some(coords) = self.missing_chunks.pop_front() else {
                break;
            };

            // Le chunk a pu être fourni entre-temps (chunk distant reçu du
            // serveur) : inutile de le regénérer.
            if self.chunks.contains_key(&coords) {
                continue;
            }

            match self.chunk_generator.request(&coords) {
                Ok(_) => {
                    self.in_flight.insert(coords);
                }
                Err(_) => {
                    // La file du générateur est pleine : on remet le chunk en
                    // tête de liste et on arrête.
                    self.missing_chunks.push_front(coords);
                    break;
                }
            }
        }
    }

    /// Reçoit un chunk complet depuis le serveur et remplace le chunk local.
    pub fn apply_remote_chunk(&mut self, position: &ChunkPosition, data: &[u8]) {
        let blocks: Vec<BlockInstance> = bincode::deserialize(data).expect("Échec de désérialisation du chunk reçu");
        let chunk = Chunk {
            blocks,
            x: position.x(),
            y: position.y(),
            z: position.z(),
        };
        let chunk_data = ChunkData::new(chunk);
        self.chunks.insert(*position, chunk_data);
        // Le chunk vient du serveur : il n'est plus à générer.
        self.in_flight.remove(&position);
        self.missing_chunks.retain(|p| p != position);

        if self.are_all_neighbors_ready(position) {
            let data = MeshRequestAdd::new(*position, self.get_mesh_snapshot(position));
            self.mesh_request.add.insert(data);
        } else {
            self.waiting_to_mesh.push_back(*position);
        }

        for (nx, ny, nz) in Chunk::neighbors_from_block_pos(position.x(), position.y(), position.z()) {
            self.mark_for_remesh(ChunkPosition::new(nx, ny, nz));
        }

        // for (dx, dy, dz) in DIRECT_NORMALS_3D {
        //     let (cx, cy, cz) = (cx + dx, cy + dy, cz + dz);
        //     let coords = (cx, cy, cz);
        //     if self.chunks.contains_key(&coords) {
        //         let data = MeshRequestAdd::new(*position, self.get_mesh_snapshot(position));
        //         self.mesh_request.add.insert(data);
        //     }
        // }
    }

    fn compute_generated_chunks(&mut self) {
        while let Some(result) = self.chunk_generator.try_recv() {
            let (chunk_position, chunk_with_checksum) = result.output;

            // Le chunk a été généré : il n'est plus en vol.
            self.in_flight.remove(&chunk_position);

            if self.chunks.contains_key(&chunk_position) {
                continue;
            }

            let mut chunk = chunk_with_checksum.chunk_data;
            chunk.is_dirty = false;
            self.chunks.insert(chunk_position, chunk);
            self.waiting_to_mesh.push_back(chunk_position);
        }
    }

    fn mark_for_remesh(&mut self, position: ChunkPosition) {
        if let Some(chunk) = self.chunks.get_mut(&position) {
            chunk.set_dirty();
            self.waiting_to_mesh.push_back(position);
        }
    }

    fn submit_to_mesh(&mut self) {
        if self.waiting_to_mesh.is_empty() {
            return;
        }

        let mut waiting = mem::replace(&mut self.waiting_to_mesh, UniqueQueue::new());

        waiting.retain(|chunk| {
            // Le chunk a pu être nettoyé (sorti de la zone du joueur) depuis
            // sa génération : il n'existe plus, il n'a plus lieu d'être
            // dans la file. Sans cette purge, la file grossirait
            // indéfiniment au fil des déplacements du joueur.
            if !self.chunks.contains_key(chunk) {
                return false;
            }

            if self.are_all_neighbors_ready(chunk) {
                let data = MeshRequestAdd::new(*chunk, self.get_mesh_snapshot(chunk));
                self.mesh_request.add.insert(data);
                if let Some(chunk) = self.chunks.get_mut(chunk) {
                    chunk.is_dirty = false;
                }
                false
            } else {
                true
            }
        });

        self.waiting_to_mesh = waiting;
    }

    pub fn get_mesh_snapshot(&self, chunk_position: &ChunkPosition) -> MeshSnapshot {
        let (cx, cy, cz) = chunk_position.into();
        MeshSnapshot {
            main: Arc::clone(&self.chunks.get(&chunk_position).unwrap().chunk),
            neg_x: self
                .chunks
                .get(&ChunkPosition::new(cx - 1, cy, cz))
                .map(|d| Arc::clone(&d.chunk)),
            neg_y: self
                .chunks
                .get(&ChunkPosition::new(cx, cy - 1, cz))
                .map(|d| Arc::clone(&d.chunk)),
            neg_z: self
                .chunks
                .get(&ChunkPosition::new(cx, cy, cz - 1))
                .map(|d| Arc::clone(&d.chunk)),
            pos_x: self
                .chunks
                .get(&ChunkPosition::new(cx + 1, cy, cz))
                .map(|d| Arc::clone(&d.chunk)),
            pos_y: self
                .chunks
                .get(&ChunkPosition::new(cx, cy + 1, cz))
                .map(|d| Arc::clone(&d.chunk)),
            pos_z: self
                .chunks
                .get(&ChunkPosition::new(cx, cy, cz + 1))
                .map(|d| Arc::clone(&d.chunk)),
        }
    }

    pub fn are_all_neighbors_ready(&self, chunk_position: &ChunkPosition) -> bool {
        let (cx, cy, cz) = chunk_position.into();

        // Requires that every direct chunk neighbor's state is Ready.
        // If a neighbor chunk is missing, it is considered blocking to avoid inter-chunks faces.
        for (dx, dy, dz) in DIRECT_NORMALS_3D {
            if let Some(neighbor) = self.chunks.get(&ChunkPosition::new(cx + dx, cy + dy, cz + dz)) {
                if neighbor.state != ChunkState::Ready {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn print_memory(&self) {
        let conversion = |b: u32| {
            let kb = b / 1024;
            let mb = kb / 1024;
            (mb, kb, b)
        };

        let chunk_size = size_of::<Chunk>() + size_of::<[BlockInstance; CHUNK_BLOCK_NUMBER]>() - size_of::<Vec<Chunk>>();
        let chunks_capacity = (chunk_size * self.chunks.capacity()) as u32;
        let chunks_length = (chunk_size * self.chunks.len()) as u32;

        let chunk_alloc = chunks_capacity;
        let (chunk_alloc_mb, chunk_alloc_kb, chunk_alloc_b) = conversion(chunk_alloc);

        let chunk_free = chunks_capacity - chunks_length;
        let (chunk_free_mb, chunk_free_kb, chunk_free_b) = conversion(chunk_free);

        let chunk_used = chunks_length;
        let (chunk_used_mb, chunk_used_kb, chunk_used_b) = conversion(chunk_used);

        println!(
            "Chunk count/capacity: {}/{} ({:3.1}%)",
            self.chunks.len(),
            self.chunks.capacity(),
            (100f32 * self.chunks.len() as f32) / (self.chunks.capacity() as f32)
        );
        println!(
            "Allocated Memory (CPU) {:3}Mb | {:6}Kb | {:9}b",
            chunk_alloc_mb, chunk_alloc_kb, chunk_alloc_b
        );
        println!(
            "└─ Free                {:3}Mb | {:6}Kb | {:9}b",
            chunk_free_mb, chunk_free_kb, chunk_free_b
        );
        println!(
            "└─ Used                {:3}Mb | {:6}Kb | {:9}b",
            chunk_used_mb, chunk_used_kb, chunk_used_b
        );
    }

    pub fn dispose(&mut self) {
        self.chunks.clear();
        self.waiting_to_mesh.clear();
        self.missing_chunks.clear();
        self.in_flight.clear();
        // TODO: faire fonctionner -> self.block_manager.dispose();
        self.chunk_generator.dispose();
    }

    pub fn set_item_manager(&mut self, item_manager: Arc<RwLock<ItemManager>>) {
        self.item_manager = item_manager;
    }
}

impl CollisionWorld for ClientWorld {
    fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn is_block_solid(&self, position: &WorldPosition) -> bool {
        let chunk = position.into();
        if !self.chunks.contains_key(&chunk) {
            return true;
        }
        self.get_block(position).map_or(false, |block| block.is_solid())
    }
}

#[derive(Debug)]
pub struct ClientChunks {
    inner: FxHashMap<ChunkPosition, ChunkData>,
}

impl ClientChunks {
    pub const fn new() -> Self {
        Self {
            inner: HashMap::with_hasher(FxBuildHasher),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn keys(&self) -> Keys<'_, ChunkPosition, ChunkData> {
        self.inner.keys()
    }

    fn insert(&mut self, position: ChunkPosition, chunk_data: ChunkData) -> Option<ChunkData> {
        self.inner.insert(position, chunk_data)
    }
}

impl ChunkStorage for ClientChunks {
    type Value = ChunkData;

    fn get(&self, position: &ChunkPosition) -> Option<&ChunkData> {
        self.inner.get(position)
    }

    fn get_mut(&mut self, position: &ChunkPosition) -> Option<&mut ChunkData> {
        self.inner.get_mut(position)
    }

    fn contains_key(&self, coords: &ChunkPosition) -> bool {
        self.inner.contains_key(coords)
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn extend<T: Iterator<Item = (ChunkPosition, ChunkData)>>(&mut self, iter: T) {
        self.inner.extend(iter);
    }

    fn retain<F: FnMut(&ChunkPosition, &mut ChunkData) -> bool>(&mut self, f: F) {
        self.inner.retain(f);
    }
}

impl World for ClientWorld {
    type Storage = ClientChunks;

    fn seed(&self) -> u32 {
        self.seed
    }

    fn chunks(&self) -> &Self::Storage {
        &self.chunks
    }

    fn chunks_mut(&mut self) -> &mut Self::Storage {
        &mut self.chunks
    }

    fn chunk_generator(&self) -> &ChunkGenerator {
        &self.chunk_generator
    }

    fn block_manager(&self) -> &Arc<RwLock<BlockManager>> {
        &self.block_manager
    }

    fn item_manager(&self) -> &Arc<RwLock<ItemManager>> {
        &self.item_manager
    }

    fn set_block(&mut self, position: &WorldPosition, block: BlockInstance) -> Result<(), ChunkPosition> {
        let (chunk_position, local_position): (ChunkPosition, LocalPosition) = position.into();

        let Some(chunk) = self.chunks.get_mut(&chunk_position) else {
            log_warn_server!("Tentative de modification d'un chunk non chargé : {:?}", chunk_position);
            return Err(chunk_position);
        };

        if chunk.set_block(&local_position, block).is_none() {
            return Ok(());
        }

        self.mark_for_remesh(chunk_position);

        let (x, y, z) = position.into();
        for (nx, ny, nz) in Chunk::neighbors_from_block_pos(x, y, z) {
            self.mark_for_remesh(ChunkPosition::new(nx, ny, nz));
        }

        Ok(())
    }
}
