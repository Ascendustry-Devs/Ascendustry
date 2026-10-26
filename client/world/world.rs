use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use engine::render::modes::RenderMode;
use num_cpus::get;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::api::texture_loader::TextureLoader;
use cgmath::Point3;
use game::constants::{
    DIRECT_NORMALS_3D, HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F, MAX_GENERATION_CHUNKS_IN_QUEUE,
    SIMULATION_DISTANCE_CHUNK_COUNT,
};
use game::inventory::item_manager::ItemManager;
use game::inventory::ItemInstance;
use game::world::data::block::{BlockInstance, BlockManager};
use game::world::data::chunk::{Chunk, ChunkData, ChunkState, CHUNK_BLOCK_NUMBER, CHUNK_SIZE, CHUNK_SIZE_HALFED_VEC3_F};
use game::world::generation::chunk_generator::ChunkGenerator;
use game::world::scheduling::compute_missing_chunks_sorted;
use physics::aabb::AABB;
use physics::collision_world::CollisionWorld;
use project_core::general_types::unique_queue::{FxUniqueQueue, UniqueQueue};
use std::cmp::min;
use std::collections::HashSet;
use std::{
    cmp::max,
    collections::{HashMap, VecDeque},
    mem,
    sync::{Arc, RwLock},
};

use crate::{
    player::player::Player,
    render::meshing::world::WorldMesh,
    world::mesh_request::{MeshRequestAdd, MeshRequestMessage, MeshResponse, MeshSnapshot},
};

pub struct World {
    seed: u32,
    chunks: FxHashMap<(i32, i32, i32), ChunkData>,
    chunk_generator: ChunkGenerator,
    block_manager: Arc<RwLock<BlockManager>>,
    item_manager: Arc<RwLock<ItemManager>>,
    /// Chunks déjà générés mais pas encore maillés (il manque des voisins).
    waiting_to_mesh: FxUniqueQueue<(i32, i32, i32)>,
    /// Chunks restant à générer, triés par priorité (distance au joueur).
    ///
    /// Cette liste est **persistante** : les chunks qui n'ont pas pu être
    /// demandés au générateur (file pleine) y restent et seront servis dès
    /// que de la place se libère, au lieu d'être perdus.
    missing_chunks: VecDeque<(i32, i32, i32)>,
    /// Chunks demandés au générateur mais pas encore reçus.
    ///
    /// Sert à ne pas demander deux fois le même chunk (le générateur étant
    /// asynchrone, un chunk en cours de génération n'est pas encore présent
    /// dans `chunks`).
    in_flight: FxHashSet<(i32, i32, i32)>,
    mesh_request: MeshRequestMessage,
}

impl World {
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
            chunks: HashMap::with_hasher(FxBuildHasher),
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
            let items = game::assets::item_loader::load_item_definitions(game::assets::resolve_asset_path("items/"))
                .expect("Failed to load item definitions");
            for mut item in items {
                if let Some(tex_path) = &item.texture_path {
                    if let Ok(tex_id) = texture_loader.register(tex_path.clone(), RenderMode::Opaque) {
                        item.texture_index = Some(tex_id.id());
                    }
                }
                item_manager.register(item);
            }
        }

        {
            let mut block_manager = self.block_manager.write().unwrap();
            let item_manager = self.item_manager.read().unwrap();

            let mut defs = game::assets::block_loader::load_block_definitions(game::assets::resolve_asset_path("blocks/"))
                .expect("Failed to load block definitions");

            for def in &mut defs {
                if let Some(tex_path) = &def.texture_path {
                    let render_mode = match def.render_mode.to_lowercase().as_str() {
                        "opaque" => RenderMode::Opaque,
                        _ => RenderMode::Opaque,
                    };
                    if let Ok(tex_id) = texture_loader.register(tex_path.clone(), render_mode) {
                        def.texture_index = Some(tex_id.id());
                    }
                }
                block_manager.register(def.clone());
            }

            block_manager.resolve_item_mappings(&item_manager);
            block_manager.build_texture_lookup();
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
            let (cx, cy, cz) = response;
            if let Some(chunk) = self.get_chunk_data_mut(cx, cy, cz) {
                chunk.state = ChunkState::Ready;
            }
        }
    }

    pub fn chunk_infos_at(&self, cpos: &(i32, i32, i32)) -> Option<(ChunkState, bool)> {
        self.chunks.get(cpos).map(|chunk| chunk.get_debug_infos())
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn get_texture_lookup(&self) -> Arc<Vec<u32>> {
        let bm = self.block_manager.read().unwrap();
        Arc::new(bm.get_texture_lookup().to_vec())
    }

    pub fn block_to_item(&self, block_id: u32) -> Option<ItemInstance> {
        let bm = self.block_manager.read().unwrap();
        let im = self.item_manager.read().unwrap();
        bm.block_to_item(block_id, &im)
    }

    pub fn item_to_block(&self, item_id: u32) -> Option<u32> {
        self.block_manager.read().unwrap().item_to_block_id(item_id)
    }

    pub const fn seed(&self) -> u32 {
        self.seed
    }

    pub fn get_item_manager(&self) -> Arc<RwLock<ItemManager>> {
        Arc::clone(&self.item_manager)
    }

    fn clean_chunks(&mut self, mesh_manager: &Arc<RwLock<GpuAllocator>>, world_mesh: &mut WorldMesh, player: &Player) {
        let alloc = &mut mesh_manager.write().unwrap();

        // Coordonnées du chunk en blocks où se trouve le joueur
        let chunk_pos_in_blocks = player.get_cpos().map(|coord| (coord * CHUNK_SIZE) as f32);
        let player_simulation_aabb = AABB::new_sized(chunk_pos_in_blocks, HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F);

        self.chunks.retain(|key, _| {
            let (cx, cy, cz) = *key;
            let key_vec_f = Point3::new(cx, cy, cz).map(|coord| (coord * CHUNK_SIZE) as f32);
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
        let player_chunk: (i32, i32, i32) = player.get_cpos().into();

        let missing = compute_missing_chunks_sorted(
            player.get_simulation_chunk_keys(),
            &self.chunks.keys().copied().collect(),
            &self.in_flight,
            player_chunk,
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

            let (cx, cy, cz) = coords;
            match self.chunk_generator.request(cx, cy, cz) {
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
    pub fn apply_remote_chunk(&mut self, cx: i32, cy: i32, cz: i32, data: &[u8]) {
        let blocks: Vec<BlockInstance> = bincode::deserialize(data).expect("Échec de désérialisation du chunk reçu");
        let chunk = Chunk {
            blocks,
            x: cx,
            y: cy,
            z: cz,
        };
        let chunk_data = ChunkData::new(chunk);
        self.chunks.insert((cx, cy, cz), chunk_data);
        // Le chunk vient du serveur : il n'est plus à générer.
        self.in_flight.remove(&(cx, cy, cz));
        self.missing_chunks.retain(|coords| *coords != (cx, cy, cz));

        let data = MeshRequestAdd::new((cx, cy, cz), self.get_mesh_snapshot(cx, cy, cz));
        self.mesh_request.add.insert(data);
        for (dx, dy, dz) in DIRECT_NORMALS_3D {
            let (cx, cy, cz) = (cx + dx, cy + dy, cz + dz);
            let coords = (cx, cy, cz);
            if self.chunks.contains_key(&coords) {
                let data = MeshRequestAdd::new(coords, self.get_mesh_snapshot(cx, cy, cz));
                self.mesh_request.add.insert(data);
            }
        }
    }

    fn compute_generated_chunks(&mut self) {
        while let Some(result) = self.chunk_generator.try_recv() {
            let (cx, cy, cz, chunk_with_checksum) = result.output;
            let key = (cx, cy, cz);

            // Le chunk a été généré : il n'est plus en vol.
            self.in_flight.remove(&key);

            if self.chunks.contains_key(&key) {
                continue;
            }

            let mut chunk = chunk_with_checksum.chunk_data;
            chunk.is_dirty = false;
            self.chunks.insert(key, chunk);
            self.waiting_to_mesh.push_back(key);
        }
    }

    fn submit_to_mesh(&mut self) {
        if self.waiting_to_mesh.is_empty() {
            return;
        }

        let mut waiting = mem::replace(&mut self.waiting_to_mesh, UniqueQueue::new());

        waiting.retain(|chunk| {
            let chunk = *chunk;
            let (cx, cy, cz) = chunk;

            // Le chunk a pu être nettoyé (sorti de la zone du joueur) depuis
            // sa génération : il n'existe plus, il n'a plus lieu d'être
            // dans la file. Sans cette purge, la file grossirait
            // indéfiniment au fil des déplacements du joueur.
            if !self.chunks.contains_key(&chunk) {
                return false;
            }

            if self.are_all_neighbors_ready(cx, cy, cz) {
                let data = MeshRequestAdd::new(chunk, self.get_mesh_snapshot(cx, cy, cz));
                self.mesh_request.add.insert(data);
                false
            } else {
                true
            }
        });

        self.waiting_to_mesh = waiting;
    }

    pub fn get_mesh_snapshot(&self, cx: i32, cy: i32, cz: i32) -> MeshSnapshot {
        MeshSnapshot {
            main: Arc::clone(&self.chunks.get(&(cx, cy, cz)).unwrap().chunk),
            neg_x: self.chunks.get(&(cx - 1, cy, cz)).map(|d| Arc::clone(&d.chunk)),
            neg_y: self.chunks.get(&(cx, cy - 1, cz)).map(|d| Arc::clone(&d.chunk)),
            neg_z: self.chunks.get(&(cx, cy, cz - 1)).map(|d| Arc::clone(&d.chunk)),
            pos_x: self.chunks.get(&(cx + 1, cy, cz)).map(|d| Arc::clone(&d.chunk)),
            pos_y: self.chunks.get(&(cx, cy + 1, cz)).map(|d| Arc::clone(&d.chunk)),
            pos_z: self.chunks.get(&(cx, cy, cz + 1)).map(|d| Arc::clone(&d.chunk)),
        }
    }

    #[inline(always)]
    pub fn get_chunk_data(&self, cx: i32, cy: i32, cz: i32) -> Option<&ChunkData> {
        self.chunks.get(&(cx, cy, cz))
    }

    #[inline(always)]
    pub fn get_chunk_data_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Option<&mut ChunkData> {
        self.chunks.get_mut(&(cx, cy, cz))
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) -> bool {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        let (lx, ly, lz) = Chunk::local_coords_from_world(x, y, z);
        let Some(chunk) = self.get_chunk_data_mut(cx, cy, cz) else {
            return false;
        };
        let current_block = chunk.chunk.get_block_from_xyz(lx, ly, lz);
        if current_block == block {
            false
        } else {
            Arc::make_mut(&mut chunk.chunk).set_block_from_xyz(lx, ly, lz, block);

            let data = MeshRequestAdd::new((cx, cy, cz), self.get_mesh_snapshot(cx, cy, cz));
            self.mesh_request.add.insert(data);
            for (ncx, ncy, ncz) in Chunk::neighbors_from_block_pos(x, y, z) {
                if self.get_chunk_data(ncx, ncy, ncz).is_some() {
                    let data = MeshRequestAdd::new((ncx, ncy, ncz), self.get_mesh_snapshot(ncx, ncy, ncz));
                    self.mesh_request.add.insert(data);
                }
            }
            true
        }
    }

    pub fn get_block_from_xyz(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        let (lx, ly, lz) = Chunk::local_coords_from_world(x, y, z);

        self.get_chunk_data(cx, cy, cz)
            .map_or_else(BlockInstance::air, |data| data.chunk.get_block_from_xyz(lx, ly, lz))
    }

    pub fn are_all_neighbors_ready(&self, cx: i32, cy: i32, cz: i32) -> bool {
        // Requires that every direct chunk neighbor's state is Ready.
        // If a neighbor chunk is missing, it is considered blocking to avoid inter-chunks faces.
        for (dx, dy, dz) in DIRECT_NORMALS_3D {
            if let Some(neighbor) = self.chunks.get(&(cx + dx, cy + dy, cz + dz)) {
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
}

impl CollisionWorld for World {
    fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn is_block_solid(&self, x: i32, y: i32, z: i32) -> bool {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        if !self.chunks.contains_key(&(cx, cy, cz)) {
            return true;
        }
        self.get_block_from_xyz(x, y, z).is_solid()
    }
}
