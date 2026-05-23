use engine::render::{mesh::manager::MeshManager, texture::RenderMode};

use crate::api::texture_loader::TextureLoader;
use cgmath::Point3;
use physics::aabb::AABB;
use physics::collision_world::CollisionWorld;
use satiscore::{
    constants::DIRECT_NORMALS_3D,
    utils::unique_queue::{FastUniqueQueue, UniqueQueue},
    world::data::block::BlockData,
};
use satiscore::{constants::MAX_GENERATION_CHUNKS_IN_QUEUE, log_err_client};
use satiscore::{
    constants::SIMULATION_DISTANCE_IN_BLOCKS_HALFED_VEC3_F,
    world::{
        data::{
            block::{BlockInstance, BlockManager},
            chunk::{Chunk, ChunkData, ChunkState, CHUNK_SIZE, CHUNK_SIZE_HALFED_VEC3_F},
        },
        generation::chunk_generator::ChunkGenerator,
    },
};
use std::{
    cmp::max,
    collections::HashMap,
    mem,
    sync::{Arc, RwLock},
};

use crate::{
    // engine::render::mesh::manager::RenderManager,
    player::player::Player,
    render::meshing::world::WorldMesh,
};

#[derive(Clone)]
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
    chunks: HashMap<(i32, i32, i32), ChunkData>,
    chunk_generator: ChunkGenerator,
    block_manager: Arc<RwLock<BlockManager>>,
    waiting_to_mesh: FastUniqueQueue<(i32, i32, i32)>,
    ready_to_mesh: FastUniqueQueue<(i32, i32, i32)>,
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
            chunks: HashMap::new(),
            chunk_generator: chunk_generator,
            block_manager: block_manager,
            waiting_to_mesh: UniqueQueue::with_capacity(256),
            ready_to_mesh: UniqueQueue::with_capacity(256),
        };
    }

    pub fn init(&mut self, texture_loader: &mut TextureLoader, player: &Player) {
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
                    values.0.texture_index = Some(tex_id.depth() as u32);
                };
                block_manager.register(values.0);
            }
        }

        self.generate_missing_chunks(player);
    }

    pub fn update(&mut self, mesh_manager: &mut MeshManager, world_mesh: &mut WorldMesh, player: &Player) {
        if player.state.cpos.has_changed() || self.chunks.is_empty() {
            self.clean_chunks(mesh_manager, world_mesh, player);
            self.generate_missing_chunks(player);
        }
        self.compute_generated_chunks();
        self.submit_to_mesh();
    }

    fn clean_chunks(&mut self, mesh_manager: &mut MeshManager, world_mesh: &mut WorldMesh, player: &Player) {
        // Maths
        // let radii_h_squared = (player.state.horizontal_render_distance * player.state.horizontal_render_distance) as i32;
        // let radii_v_squared = (player.state.vertical_render_distance * player.state.vertical_render_distance) as i32;

        // Coordonnées du chunk où se trouve le joueur (entiers)
        let cpos = player.get_cpos().map(|coord| coord as f32);
        let player_simulation_aabb = AABB::new_sized(cpos, SIMULATION_DISTANCE_IN_BLOCKS_HALFED_VEC3_F);

        self.chunks.retain(|key, _| {
            let (cx, cy, cz) = *key;
            let key_vec_f = Point3::new(cx as f32, cy as f32, cz as f32);
            let chunk_aabb = AABB::new_sized(key_vec_f, CHUNK_SIZE_HALFED_VEC3_F);
            // On garde les chunks qui sont à portée du joueur
            if chunk_aabb.overlaps(&player_simulation_aabb) {
                return true;
            }
            // On supprime proprement ceux qui ne le sont plus
            let Some(mesh) = world_mesh.meshes.remove(key) else {
                return false;
            };
            let Some(id) = mesh.id else {
                return false;
            };
            let result = mesh_manager.free(id);
            match result {
                Ok(_) => {}
                Err(err) => {
                    let (x, y, z) = key;
                    log_err_client!(
                        "World: an error occured while trying to free chunk (x: {} y: {} z: {})'s mesh with id {}.\nError: {}",
                        x,
                        y,
                        z,
                        id,
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

    fn compute_generated_chunks(&mut self) {
        while let Some(result) = self.chunk_generator.try_recv() {
            let (cx, cy, cz, chunk_with_checksum) = result.output;
            let key = (cx, cy, cz);

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
                self.ready_to_mesh.push_back(chunk);
                false
            } else {
                true
            }
        });

        self.waiting_to_mesh = waiting;
    }

    pub fn ready_to_mesh(&mut self) -> &mut FastUniqueQueue<(i32, i32, i32)> {
        &mut self.ready_to_mesh
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

            self.ready_to_mesh.push_back((cx, cy, cz));
            for (ncx, ncy, ncz) in Chunk::neighbors_from_block_pos(x, y, z) {
                if self.get_chunk_data(ncx, ncy, ncz).is_some() {
                    self.ready_to_mesh.push_back((ncx, ncy, ncz));
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

#[inline(always)]
fn is_chunk_in_range_radii(c: &(i32, i32, i32), center: &(i32, i32, i32), radius_h_squared: i32, radius_v_squared: i32) -> bool {
    let dx = c.0 - center.0;
    let dy = c.1 - center.1;
    let dz = c.2 - center.2;

    (dx * dx) / radius_h_squared + (dy * dy) / radius_v_squared + (dz * dz) / radius_h_squared <= 1
}
