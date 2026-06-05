use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use engine::render::modes::RenderMode;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::hash::{Hash, Hasher};

use crate::api::texture_loader::TextureLoader;
use cgmath::Point3;
use game::constants::{DIRECT_NORMALS_3D, HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F, MAX_GENERATION_CHUNKS_IN_QUEUE};
use game::world::data::block::BlockData;
use game::world::data::block::{BlockInstance, BlockManager};
use game::world::data::chunk::{Chunk, ChunkData, ChunkState, CHUNK_SIZE, CHUNK_SIZE_HALFED_VEC3_F};
use game::world::generation::chunk_generator::ChunkGenerator;
use physics::aabb::AABB;
use physics::collision_world::CollisionWorld;
use project_core::{
    log_err_client,
    utils::unique_queue::{FxUniqueQueue, UniqueQueue},
};
use std::collections::HashSet;
use std::{
    cmp::max,
    collections::HashMap,
    mem,
    sync::{Arc, RwLock},
};

use crate::{player::player::Player, render::meshing::world::WorldMesh};

#[derive(Clone, PartialEq)]
pub struct MeshSnapshot {
    pub main: Arc<Chunk>,
    pub neg_x: Option<Arc<Chunk>>,
    pub pos_x: Option<Arc<Chunk>>,
    pub neg_y: Option<Arc<Chunk>>,
    pub pos_y: Option<Arc<Chunk>>,
    pub neg_z: Option<Arc<Chunk>>,
    pub pos_z: Option<Arc<Chunk>>,
}

pub struct World {
    seed: u32,
    chunks: FxHashMap<(i32, i32, i32), ChunkData>,
    chunk_generator: ChunkGenerator,
    block_manager: Arc<RwLock<BlockManager>>,
    waiting_to_mesh: FxUniqueQueue<(i32, i32, i32)>,
    mesh_request: MeshRequestMessage,
}

pub type MeshResponse = (i32, i32, i32);

pub type MeshRequestDelete = (i32, i32, i32);

#[derive(Clone)]
pub struct MeshRequestAdd {
    pub coords: (i32, i32, i32),
    pub snapshot: MeshSnapshot,
}

impl MeshRequestAdd {
    pub fn new(coords: (i32, i32, i32), snapshot: MeshSnapshot) -> Self {
        Self { coords, snapshot }
    }
}

impl PartialEq for MeshRequestAdd {
    fn eq(&self, other: &Self) -> bool {
        self.coords == other.coords
    }
}
impl Eq for MeshRequestAdd {}
impl Hash for MeshRequestAdd {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.coords.hash(state);
    }
}

pub struct MeshRequestMessage {
    pub add: FxHashSet<MeshRequestAdd>,
    pub delete: FxHashSet<MeshRequestDelete>,
}

impl MeshRequestMessage {
    pub fn empty() -> Self {
        Self {
            add: HashSet::with_hasher(FxBuildHasher),
            delete: HashSet::with_hasher(FxBuildHasher),
        }
    }
}

impl World {
    pub fn new(seed: u32) -> World {
        let block_manager = Arc::new(RwLock::new(BlockManager::new()));

        let worker_count = max((num_cpus::get() as f32 / 2.0).floor() as usize, 1);
        let chunk_generator = ChunkGenerator::with_max_pending(
            worker_count,
            Arc::clone(&block_manager),
            seed,
            MAX_GENERATION_CHUNKS_IN_QUEUE as usize,
        );

        return World {
            seed: seed,
            chunks: HashMap::with_hasher(FxBuildHasher),
            chunk_generator: chunk_generator,
            block_manager: block_manager,
            waiting_to_mesh: UniqueQueue::with_capacity(256),
            mesh_request: MeshRequestMessage::empty(),
        };
    }

    pub fn init(&mut self, texture_loader: &mut TextureLoader, player: &Player) -> &mut MeshRequestMessage {
        {
            let mut block_manager = self.block_manager.write().unwrap();

            let blocks = [
                (BlockData::new("air"), ""),
                (BlockData::new("stone"), "assets/images/stone.png"),
                (BlockData::new("dirt"), "assets/images/dirt.png"),
                (BlockData::new("grass"), "assets/images/grass.png"),
            ];

            for mut values in blocks {
                if let Ok(tex_id) = texture_loader.register(values.1.to_string(), RenderMode::Opaque) {
                    values.0.texture_index = Some(tex_id.id());
                };
                block_manager.register(values.0);
            }
        }

        self.generate_missing_chunks(player);
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
            self.generate_missing_chunks(player);
        }
        self.compute_generated_chunks();
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

    fn clean_chunks(&mut self, mesh_manager: &mut Arc<RwLock<GpuAllocator>>, world_mesh: &mut WorldMesh, player: &Player) {
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
                    log_err_client!(
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

    fn generate_missing_chunks(&mut self, player: &Player) {
        // Si la file d'attente est pleine, ça ne sert à rien d'essayer de soumettre des demandes
        if self.chunk_generator.is_queue_full() {
            return;
        }

        // Identifier les chunks manquants
        let mut missing_keys: Vec<(i32, i32, i32)> = {
            let mut needed_keys = player.get_simulation_chunk_keys();
            needed_keys.retain(|key| !self.chunks.contains_key(key));
            needed_keys
        };

        // Coordonnées joueur (entiers)
        let (px, py, pz) = player.get_pos().map(|coord| coord.floor() as i32).into();

        // Trier les demandes de chunks manquants en fonction de leur distance au joueur (proche = prioritaire)
        missing_keys.sort_by_key(|chunk| {
            // Chaque coordonnée représente la position du chunk dans le monde, à l'échelle d'un bloc, et centré sur le joueur
            let x = chunk.0 * CHUNK_SIZE - px;
            let y = chunk.1 * CHUNK_SIZE - py;
            let z = chunk.2 * CHUNK_SIZE - pz;

            // Distance au carrée
            x * x + y * y + z * z
        });

        // On soumet les clés des chunks manquants pour les faire générer
        let mut removed = 0;
        for chunk in missing_keys.iter() {
            let (cx, cy, cz) = *chunk;
            let result = self.chunk_generator.request(cx, cy, cz);
            match result {
                Ok(_) => {
                    removed += 1;
                }
                Err(_) => {
                    // La file d'attente est pleine, on arrête ici pour l'instant
                    log_err_client!("World chunk generation: queue is full!");
                    break;
                }
            }
        }

        if removed > 0 {
            missing_keys.drain(0..removed);
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
        return self.chunks.get(&(cx, cy, cz));
    }

    #[inline(always)]
    pub fn get_chunk_data_mut(&mut self, cx: i32, cy: i32, cz: i32) -> Option<&mut ChunkData> {
        return self.chunks.get_mut(&(cx, cy, cz));
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockInstance) -> bool {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        let (lx, ly, lz) = Chunk::local_coords_from_world(x, y, z);
        let Some(chunk) = self.get_chunk_data_mut(cx, cy, cz) else {
            return false;
        };
        let current_block = chunk.chunk.get_block_from_xyz(lx, ly, lz);
        if current_block == block {
            return false;
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
            return true;
        }
    }

    pub fn get_block_from_xyz(&self, x: i32, y: i32, z: i32) -> BlockInstance {
        let (cx, cy, cz) = Chunk::chunk_coords_from_world(x, y, z);
        let (lx, ly, lz) = Chunk::local_coords_from_world(x, y, z);

        if let Some(data) = self.get_chunk_data(cx, cy, cz) {
            return data.chunk.get_block_from_xyz(lx, ly, lz);
        } else {
            return BlockInstance::air();
        }
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

    pub fn chunk_infos_at(&self, cpos: &(i32, i32, i32)) -> Option<(ChunkState, bool)> {
        self.chunks.get(&cpos).map(|chunk| chunk.get_debug_infos())
    }

    /// Retourne vrai si aucun chunk n'est chargé
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn dispose(&mut self) {
        self.chunks.clear();
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
