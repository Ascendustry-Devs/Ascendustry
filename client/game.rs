use crate::api::texture_loader::TextureLoader;
use crate::network::NetworkManager;
use crate::player::controllers::spectator::FreeCameraController;
use crate::player::controllers::walk::WalkPlayerController;
use crate::player::player::Player;
use crate::player::remote_players::RemotePlayersManager;
use crate::render::meshing::world::WorldMesh;
use crate::systems::inputs::InputState;
use crate::world::world::{MeshRequestMessage, World};
use bytemuck::cast_slice;
use cgmath::{dot, EuclideanSpace, Point3, Vector3};
use engine::audio::GameAudioManager;
use engine::core::application::AppState;
use engine::core::frame::EngineFrameData;
use engine::core::frame::GameFrameData;
use engine::geometry::vertex::generate_cube;
use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use engine::render::render::Renderer;
use engine::render::ui::interpreter::compiler::UiCompiler;
use engine::render::ui::interpreter::translator::UiTranslator;
use engine::render::ui::widgets::panel::Panel;
use engine::render::ui::widgets::{Widget, WidgetTransform};
use game::constants::{CHUNK_VECTOR, HORIZONTAL_RENDER_DISTANCE, VERTICAL_RENDER_DISTANCE};
use game::world::data::block::BlockInstance;
use game::world::data::chunk::CHUNK_SIZE_F;
use network::messages::new_save_request_paquet;
use network::messages::ContenuPaquet;
use physics::aabb::AABB;
use project_core::geometry::plane::Plane;
use project_core::{log_client, log_err_client};
use rustc_hash::FxHashSet;
use std::mem;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::Instant;
use winit::keyboard::KeyCode;

const FPS_CAP: u32 = u32::MAX;
const DT_CAP: f32 = {
    if FPS_CAP == 0 {
        0.0
    } else {
        1.0 / (FPS_CAP as f32 + 0.125)
    }
};
const PING_INTERVAL: Duration = Duration::from_secs(10);

pub struct GameState {
    pub world: World,
    pub world_mesh: WorldMesh,
    pub player: Player,
    pub remote_players: RemotePlayersManager,
    pub delay_s: f32,
    pub network: Option<NetworkManager>,
    inputs: InputState,
    last_save_request: Instant,
}

impl GameState {
    pub fn new(addr: String, name: &str, player_id: u64) -> Self {
        let mut network = NetworkManager::new();

        network.connect(&addr);
        network.perform_handshake(name, player_id);
        let server_seed = network
            .get_server_seed()
            .expect("La seed n'existe pas ou est vide (serveur non lancé ? connexion échouée ? mauvaise adresse IP ?)");
        let server_player_id = network.player_id();

        Self {
            player: Player::new(
                server_player_id,
                Box::new(FreeCameraController::new(0.00390625)),
                Box::new(WalkPlayerController),
            ),
            world: World::new(server_seed),
            world_mesh: WorldMesh::new(),
            remote_players: RemotePlayersManager::new(),
            inputs: InputState::new(),
            delay_s: 0.0,
            network: Some(network),
            last_save_request: Instant::now(),
        }
    }

    #[inline(never)]
    fn update_debug_commands(&mut self, alloc: &Arc<RwLock<GpuAllocator>>) {
        // MESH MEMORY TRACKER (CPU & GPU)
        if self.inputs.take_key_pressed(KeyCode::KeyV) {
            println!("==== Mesh Memory Tracker ====");
            println!("CPU:");
            self.world_mesh.print_memory();
            println!("-----------------------------");
            println!("GPU:");
            alloc.read().unwrap().force_print_debug_infos();
            println!("=============================");
        }
        // MESH MEMORY DUMP (CPU)
        if self.inputs.is_key_pressed(KeyCode::ControlLeft) && self.inputs.is_key_pressed(KeyCode::KeyD) {
            self.inputs.take_key_pressed(KeyCode::ControlLeft);
            self.inputs.take_key_pressed(KeyCode::KeyD);
            let alloc = &alloc.read().unwrap();
            let meshes = &self.world_mesh.meshes;
            println!("==== Mesh CPU Memory Dump ====");
            for (pos, mesh) in meshes.iter() {
                println!("- Chunk {:?}", *pos);
                let Some(id) = mesh.id else {
                    continue;
                };
                let Some(data) = alloc.get_mesh_entry(id) else {
                    continue;
                };
                println!(
                    "  └─ Mesh (id: {:?}, pos: {:?}, len: {:?})",
                    data.id, data.position, data.length
                );
            }
            println!("==============================")
        }
        // CURRENT CHUNK DEBUG
        if self.inputs.take_key_pressed(KeyCode::KeyC) {
            let cpos = self.player.state.cpos.current();
            let key = (cpos[0], cpos[1], cpos[2]);
            println!("==== Chunk {:?} ====", key);
            if let Some((state, dirty)) = self.world.chunk_infos_at(&key) {
                println!("- Data (CPU):\n  └─ {}", state);
                if dirty {
                    println!("  └─ Dirty")
                }
            }
            if let Some((id, dirty)) = self.world_mesh.mesh_infos_at(&key) {
                let id = match id {
                    Some(id) => &id.to_string(),
                    None => "None",
                };
                println!("- Mesh (GPU):\n  └─ Id: {}", id);
                if dirty {
                    println!("  └─ Dirty")
                }
            };
            println!("========================")
        }

        // SWITCH GAMEMODE
        if self.inputs.take_key_pressed(KeyCode::KeyG) {
            self.player.state.switch_player_game_mode();
            println!("Switching gamemode to {}", self.player.state.game_mode);
            if let Some(ref mut net) = self.network {
                let result = net.send_gamemode_change(self.player.state.game_mode);
                match result {
                    Ok(_) => {}
                    Err(err) => {
                        log_err_client!("Failed to switch gamemode.\nError: {}", err);
                    }
                }
            }
        }
    }

    #[inline(never)]
    fn update_physics(&mut self, frame: &EngineFrameData) {
        self.player
            .physics_update(frame.dt, &mut self.inputs, &self.world, self.player.state.game_mode.clone());
    }

    #[inline(never)]
    fn update_logic(
        &mut self,
        frame: &EngineFrameData,
        mesh_manager: &mut Arc<RwLock<GpuAllocator>>,
    ) -> (Vec<network::messages::Paquet>, MeshRequestMessage) {
        let network_commands = self.player.update(frame.dt, &mut self.world, &mut self.inputs);
        let mesh_request = self.world.update(mesh_manager, &mut self.world_mesh, &self.player);
        let mesh_request = mem::replace(mesh_request, MeshRequestMessage::empty());
        (network_commands, mesh_request)
    }

    #[inline(never)]
    fn update_network(&mut self, network_commands: Vec<network::messages::Paquet>) {
        let net = self.network.as_mut().expect("NetworkManager: uninitialized.");
        if !net.is_connected() {
            log_client!("Déconnecté du serveur. Arrêt du client.");
            exit(0);
        }
        // Envoi un ping si aucun échange n'a eu lieu depuis PING_INTERVAL
        if net.get_last_communication().elapsed() >= PING_INTERVAL {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if let Err(e) = net.send_ping(timestamp) {
                log_err_client!("Échec de l'envoi du ping.\nErreur : {}", e);
            } else {
                log_client!("Ping envoyé !");
            }
        }
        // Envoi la position et rotation du joueur au serveur si elles ont changés
        if self.player.has_moved() {
            let pos = self.player.get_pos();
            let (rx, ry) = self.player.state.camera.get_rotation();
            if let Err(e) = net.send_position(pos.x, pos.y, pos.z, rx, ry) {
                log_err_client!("Échec de l'envoi de la position.\nErreur : {}", e);
            }
            self.player.state.reset_moved();
        }
        // Réception des positions des autres joueurs
        if let Ok(Some(packet)) = net.receive_packet() {
            use ContenuPaquet;
            match packet.contenu {
                ContenuPaquet::MultiplePlayerTransformation { data } => {
                    let my_id = net.player_id();
                    self.remote_players.update(data, my_id);
                }
                ContenuPaquet::GuardCorrection { data } => {
                    let my_id = net.player_id();
                    for t in &data {
                        if t.player_id == my_id {
                            self.player.state.set_position_and_rotation(t.position, t.rotation);
                        }
                    }
                }
                ContenuPaquet::DonneesMonde { chunks } => {
                    log_client!("Paquet ContenuPaquet::DonneesMonde reçu !");
                    for c in chunks {
                        self.world.apply_remote_chunk(c.x, c.y, c.z, &c.data);
                    }
                }
                ContenuPaquet::SetBlock { x, y, z, block_id } => {
                    let block = BlockInstance::new(block_id);
                    self.world.set_block(x, y, z, block);
                }
                ContenuPaquet::Kick { reason } => {
                    log_client!("Kicked by server: {}", reason);
                    exit(0);
                }
                ContenuPaquet::InventorySet { inventory } => {
                    self.player.state.inventory = inventory;
                }
                ContenuPaquet::InventoryUpdate {
                    player_id,
                    modified_slots: inventory,
                } => {
                    if player_id == net.player_id() {
                        self.player.state.inventory.update_slots(inventory);
                    }
                }
                _ => {}
            }
        }
        for command in network_commands {
            let result = net.send_packet(command);
            match result {
                Ok(_) => {}
                Err(err) => {
                    log_err_client!("Failed to send command packet.\nError: {}", err);
                }
            }
        }
        // Envoyer une demande de sauvegarde
        if self.inputs.is_key_pressed(KeyCode::Digit3) && self.last_save_request.elapsed() > Duration::from_secs(5) {
            self.last_save_request = Instant::now();
            let packet = new_save_request_paquet();
            if let Some(net) = self.network.as_mut() {
                match net.send_packet(packet) {
                    Ok(_) => {}
                    Err(err) => {
                        log_err_client!("Failed to send save request packet.\nError: {}", err);
                    }
                }
            }
        }
        // Nettoyer les joueurs distants qui n'ont pas envoyé de mise à jour depuis 30s
        self.remote_players.cleanup_stale(Duration::from_secs(30));
    }

    #[inline(never)]
    fn update_rendering(&mut self, data: &mut GameFrameData, renderer: &mut Renderer) {
        // ATTENTION: dans le futur, trouver une alternative pour mieux mettre en cache les meshs ids
        // Peut créer des bugs de rendus difficilement débuggables
        if let Some(view_proj) = self.player.state.camera.view_proj().change() {
            let (cam_x, cam_y, cam_z) = {
                let pos = self.player.state.camera.eye();
                (pos.x, pos.y, pos.z)
            };
            data.camera.update(cam_x, cam_y, cam_z, (*view_proj).into());

            self.player.state.camera.aspect.update(renderer.render_options.aspect);

            data.visible_meshes.clear();

            self.cull_chunks(&mut data.visible_meshes);
        };
        // Update renderer with remote player positions
        let alloc = &mut renderer.render_manager.world_buffer.write().unwrap();
        for p in self.remote_players.get_all_mut().iter_mut() {
            if let Some(new_pos) = p.position.change() {
                let player_data = generate_cube(new_pos.0, new_pos.1, new_pos.2);
                let raw_data = cast_slice(&player_data);
                if let Some(mesh_id) = p.mesh_id {
                    if let Some(update_err) = alloc.update(mesh_id, raw_data).err() {
                        println!("Failed to update mesh id {}.\nError: {}", mesh_id, update_err);
                    };
                } else {
                    p.mesh_id = alloc.add(raw_data).ok();
                }
            }
            data.visible_meshes.replace(p.mesh_id.unwrap());
        }
    }

    #[inline(never)]
    fn cull_chunks(&self, out: &mut FxHashSet<u32>) {
        const BASE_REGION_HEIGHT: f32 = (VERTICAL_RENDER_DISTANCE + 1) as f32;
        const BASE_REGION_WIDTH: f32 = (HORIZONTAL_RENDER_DISTANCE + 1) as f32;

        let cam_eye = *self.player.state.camera.eye();
        let cam_position_chunk_aligned = cam_eye.map(|coord| coord - coord % CHUNK_SIZE_F);
        let cam_aabb = AABB::new_sized(
            // 3. Pour chaque coin, calculer la direction en espace chunk
            cam_position_chunk_aligned,
            Vector3::new(BASE_REGION_WIDTH, BASE_REGION_HEIGHT, BASE_REGION_WIDTH) * CHUNK_SIZE_F,
        );
        let cam_eye = cam_eye.to_vec();
        let cam_forward = self.player.state.camera.forward();
        let cam_frustum = self.player.state.camera.get_frustum_planes();

        let chunks_to_render = self.player.get_rendered_chunk_keys_set();
        let meshes = &self.world_mesh.meshes;

        for key in self.world_mesh.chunk_meshes.iter() {
            if !chunks_to_render.contains(key) {
                continue;
            }

            let Some(Some(id)) = meshes.get(key).map(|mesh| mesh.id) else {
                continue;
            };

            let min = Point3::new((key.0) as f32, (key.1) as f32, (key.2) as f32) * CHUNK_SIZE_F;
            let chunk_aabb = AABB::new_from_corner_and_dir(min, CHUNK_VECTOR);

            if !chunk_aabb.overlaps(&cam_aabb) {
                continue;
            }

            let min = min.to_vec();
            let max = min + CHUNK_VECTOR;

            // First, we check simply if the chunk to render is behind the camera.
            if is_chunk_behind_camera(&min, &max, &cam_forward, &cam_eye) {
                continue;
            }

            // Second, we check if the chunk is within the field of view of the camera.
            if !is_chunk_in_camera_frustum(&min, &max, &cam_frustum) {
                continue;
            }

            // If any of the above is true, we do not render the chunk.
            // We do the frustum check lately because it is more expansive,
            // on top of this, the first check would already eliminate ~50% of the candidates.

            out.insert(id);
        }
    }
}

impl AppState for GameState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        let mut tex_loader = TextureLoader::new(&mut renderer.texture_manager);
        let meshin = self.world.init(&mut tex_loader, &self.player);
        let alloc = Arc::clone(&renderer.render_manager.world_buffer);
        self.world_mesh.init(alloc, meshin);

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                log_err_client!("Échec de la lecture du thème principal.\nErreur : {}", e);
            }
            audio.stop_main_theme();
        }

        // UI
        // In 5 steps.

        // 1. Widget tree
        // Build everything you want with it.
        let test_panel: Panel = {
            let transform = WidgetTransform::new(8, 8, 160, 130);
            let color = 0xFFCCAAAA;
            let child = None;
            Panel::new(transform, color, child)
        };

        let mut draw_commands = Vec::new();

        // 2. Draw call
        // At the top of the tree, call root.draw and give it an empty
        // Vec of DrawCommand (it will call .draw() recursively).
        test_panel.draw(&mut draw_commands);

        // 3. Translation
        // When commands are ready, we need to translate them
        // into vertices to be compatible with the shader.
        let vertices = UiTranslator::translate(draw_commands);

        // 4. Compilation
        // Transform our UiVertices into raw bytes.
        let bytes = UiCompiler::compile(vertices);

        // 5. Submit the bytes to the GPU, and voila.
        renderer.ui_renderer.update_vertices(&bytes);
    }

    fn update(&mut self, frame: &EngineFrameData, data: &mut GameFrameData, renderer: &mut Renderer) {
        // UPDATE DELAY
        self.delay_s += frame.dt;

        if self.delay_s < DT_CAP {
            spin_sleep::sleep(Duration::from_micros(((DT_CAP - self.delay_s) * 1_000_000.0) as u64));
        }

        self.delay_s -= DT_CAP;

        let mesh_manager = &mut renderer.render_manager.world_buffer;

        self.update_debug_commands(mesh_manager);

        self.update_physics(frame);

        let (network_commands, mut mesh_request) = self.update_logic(frame, mesh_manager);

        self.update_network(network_commands);

        let mut responses = self.world_mesh.update(mesh_manager, &mut mesh_request);
        self.world.listen(&mut responses);

        self.update_rendering(data, renderer);
    }

    fn on_mouse_move(&mut self, dx: f64, dy: f64) {
        self.inputs.set_mouse_delta((dx, dy));
    }

    fn on_key(&mut self, code: KeyCode, is_pressed: bool) {
        self.inputs.set_key_press(code, is_pressed);
    }

    fn dispose(&mut self, alloc: &mut Arc<RwLock<GpuAllocator>>) {
        // TODO: faire fonctionner -> // Network dispose (disconnection, memory release...), if any.
        // if let Some(net) = self.network.as_mut() {

        // }

        self.world.dispose();
        self.world_mesh.dispose(alloc);
    }
}

#[inline(never)]
fn is_chunk_behind_camera(min: &Vector3<f32>, max: &Vector3<f32>, cam_forward: &Vector3<f32>, cam_eye: &Vector3<f32>) -> bool {
    let extent = (max - min) * 0.5;
    let center = min + extent;

    let radius = extent.x * cam_forward.x.abs() + extent.y * cam_forward.y.abs() + extent.z * cam_forward.z.abs();

    let distance = dot(*cam_forward, center - *cam_eye);

    distance + radius < 0.0
}

#[inline(never)]
fn is_chunk_in_camera_frustum(min: &Vector3<f32>, max: &Vector3<f32>, planes: &[Plane; 6]) -> bool {
    for p in planes {
        let positive = Vector3::new(
            if p.normal.x >= 0.0 { max.x } else { min.x },
            if p.normal.y >= 0.0 { max.y } else { min.y },
            if p.normal.z >= 0.0 { max.z } else { min.z },
        );
        if p.distance(positive) < 0.0 {
            return false;
        }
    }
    true
}
