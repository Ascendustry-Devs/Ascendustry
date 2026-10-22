use crate::api::texture_loader::TextureLoader;
use crate::network::NetworkManager;
use crate::player::controllers::spectator::FreeCameraController;
use crate::player::controllers::walk::WalkPlayerController;
use crate::player::player::Player;
use crate::player::remote_players::RemotePlayersManager;
use crate::render::meshing::world::WorldMesh;
use crate::render::renderer::GameRenderer;
use crate::systems::inputs::InputState;
use crate::ui::ClientUI;
use crate::world::{mesh_request::MeshRequestMessage, world::World};
use engine::audio::GameAudioManager;
use engine::core::application::AppState;
use engine::core::frame::EngineFrameData;
use engine::core::frame::GameFrameData;
use engine::gpu::allocator::gpu_allocator::GpuAllocator;
use engine::render::render::Renderer;
use engine::render::ui::interpreter::compiler::UiCompiler;
use engine::render::ui::interpreter::translator::UiTranslator;
use engine::render::ui::widgets::textured_panel::TexturedPanel;
use engine::render::ui::widgets::{Widget, WidgetTransform};
use game::world::data::block::BlockInstance;
use network::messages::new_save_request_paquet;
use network::messages::ContenuPaquet;
use project_core::{log_client, log_err_client};
use std::mem;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::Instant;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

const FPS_CAP: u32 = 60; // u32::MAX;
const DT_CAP: f32 = {
    if FPS_CAP == 0 {
        0.0
    } else {
        1.0 / (FPS_CAP as f32 + 0.125)
    }
};
const PING_INTERVAL: Duration = Duration::from_secs(10);

pub static mut TIME_SPENT_MESHING_NANOS: u128 = 0;
pub static mut TIME_SPENT_PREPARING_NANOS: u128 = 0;
pub static mut CHUNKS_MESHED: u32 = 0;

#[derive(Clone, Debug, Default)]
pub struct MeshTimeStats {
    pub count: u64,
    pub sum_nanos: u128,
    pub min_nanos: u128,
    pub max_nanos: u128,
    pub last_nanos: u128,
}

pub static mut MESH_STAT_TOTAL: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_PREP: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_MASK: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_AO: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_RECT: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_EMIT: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut MESH_STAT_OTHER: MeshTimeStats = MeshTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};

// Buffer circulaire pour calculer médian/écart-type sans allocation massive
pub const MESH_HIST_LEN: usize = 4096;
pub static mut MESH_HIST_TOTAL: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_PREP: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_MASK: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_AO: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_RECT: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_EMIT: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_OTHER: [u128; MESH_HIST_LEN] = [0u128; MESH_HIST_LEN];
pub static mut MESH_HIST_IDX: usize = 0;

pub struct GameState {
    pub world: World,
    pub world_mesh: WorldMesh,
    pub player: Player,
    pub remote_players: RemotePlayersManager,
    pub delay_s: f32,
    pub network: Option<NetworkManager>,
    pub ui: ClientUI,
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
            ui: Default::default(),
            last_save_request: Instant::now(),
        }
    }

    #[inline(never)]
    fn update_debug_commands(&mut self, alloc: &Arc<RwLock<GpuAllocator>>) {
        // MESH MEMORY OVERVIEW (CPU & GPU)
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
            let alloc = alloc.read().unwrap();
            let meshes = &self.world_mesh.meshes;
            println!("==== Mesh CPU Memory Dump ====");
            for (pos, mesh) in meshes.iter() {
                println!("- Chunk {:?}", *pos);
                let Some(id) = mesh.id else {
                    continue;
                };
                let Ok(data) = alloc.get_entry(id) else {
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
                let id = id.map_or_else(|| "None".to_string(), |id| id.to_string());
                println!("- Mesh (GPU):\n  └─ Id: {}", id);
                if dirty {
                    println!("  └─ Dirty")
                }
            };
            println!("========================")
        }
        // PRINT WORLD SEED
        if self.inputs.is_key_pressed(KeyCode::ControlLeft) && self.inputs.is_key_pressed(KeyCode::KeyS) {
            self.inputs.take_key_pressed(KeyCode::ControlLeft);
            self.inputs.take_key_pressed(KeyCode::KeyS);
            log_client!("CLIENT WORLD SEED: {}", self.world.seed());
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

        if self.inputs.take_key_pressed(KeyCode::KeyO) {
            unsafe fn fmt_ns(v: u128) -> String {
                if v >= 1_000 {
                    format!("{}.{:03}µs", v / 1000, v % 1000,)
                } else {
                    format!("{}ns", v)
                }
            }
            unsafe fn med_std(hist: &[u128; MESH_HIST_LEN], count: u64) -> (u128, f64) {
                if count == 0 {
                    return (0, 0.0);
                }
                let c_usize = count as usize;
                let n = if c_usize < MESH_HIST_LEN { c_usize } else { MESH_HIST_LEN };
                let mut tmp = [0u128; MESH_HIST_LEN];
                for i in 0..n {
                    tmp[i] = hist[i];
                }
                let slice = &mut tmp[0..n];
                slice.sort_unstable_by(|a, b| a.cmp(b));
                let med = slice[n / 2];
                // mean
                let mut sum: u128 = 0;
                for i in 0..n {
                    sum += slice[i];
                }
                let mean = (sum as f64) / (n as f64);
                let mut var: f64 = 0.0;
                for i in 0..n {
                    let d = (slice[i] as f64) - mean;
                    var += d * d;
                }
                var = var / (n as f64);
                (med, var.sqrt())
            }
            unsafe fn print_stat(name: &str, s: &MeshTimeStats, hist: &[u128; MESH_HIST_LEN]) {
                if s.count == 0 {
                    println!("{}: aucun échantillon", name);
                    return;
                }
                let avg = s.sum_nanos / (s.count as u128);
                let (med, stddev) = med_std(hist, s.count);
                println!(
                    "{}: count={} sum={} avg={} min={} max={} med={} stddev={} | {}",
                    name,
                    s.count,
                    fmt_ns(s.sum_nanos),
                    fmt_ns(avg),
                    fmt_ns(s.min_nanos),
                    fmt_ns(s.max_nanos),
                    fmt_ns(med),
                    fmt_ns(stddev.floor() as u128),
                    fmt_ns(s.last_nanos)
                );
            }
            unsafe {
                let time_spent_preparing_nanos = TIME_SPENT_PREPARING_NANOS.clone();
                let time_spent_meshing_nanos = TIME_SPENT_MESHING_NANOS.clone();
                let chunks_meshed = CHUNKS_MESHED.clone();
                println!("==== MESH PROFILER (touche O) ====");
                println!(
                    "Global: preparing={}ns meshing={}ns chunks={}",
                    time_spent_preparing_nanos, time_spent_meshing_nanos, chunks_meshed
                );
                if chunks_meshed > 0 {
                    let avg_preparing_nanos = time_spent_preparing_nanos / (chunks_meshed as u128);
                    let avg_meshing_nanos = time_spent_meshing_nanos / (chunks_meshed as u128);
                    println!("  avg preparing: {}", fmt_ns(avg_preparing_nanos));
                    println!("  avg meshing:    {}", fmt_ns(avg_meshing_nanos));
                }
                println!("--- Par chunk (détails) ---");
                print_stat("TOTAL", &MESH_STAT_TOTAL, &MESH_HIST_TOTAL);
                print_stat("PREP (ChunkSolidity::new + setup)", &MESH_STAT_PREP, &MESH_HIST_PREP);
                print_stat("MASK (détection faces / plans)", &MESH_STAT_MASK, &MESH_HIST_MASK);
                print_stat("AO (face_ao)", &MESH_STAT_AO, &MESH_HIST_AO);
                print_stat("RECT (fusion greedy / rectangles)", &MESH_STAT_RECT, &MESH_HIST_RECT);
                print_stat("EMIT (construction vertices / tri)", &MESH_STAT_EMIT, &MESH_HIST_EMIT);
                print_stat("OTHER (reste / swap)", &MESH_STAT_OTHER, &MESH_HIST_OTHER);
                println!("===============================");
            }
        }
    }

    #[inline(never)]
    fn update_physics(&mut self, frame: &EngineFrameData) {
        self.player
            .physics_update(frame.dt, &mut self.inputs, &self.world, self.player.state.game_mode);
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
                } if player_id == net.player_id() => {
                    self.player.state.inventory.update_slots(inventory);
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

    #[inline(always)]
    fn update_rendering(&mut self, data: &mut GameFrameData, renderer: &mut Renderer) {
        GameRenderer::render(self, data, renderer);
    }
}

impl AppState for GameState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        let mut tex_loader = TextureLoader::new(&mut renderer.texture_manager);
        let meshin = self.world.init(&mut tex_loader, &self.player);
        let alloc = Arc::clone(&renderer.render_manager.world_buffer);
        self.world_mesh.init(alloc, meshin);
        let tex_lookup = self.world.get_texture_lookup();
        self.world_mesh.set_texture_lookup(tex_lookup);

        self.ui = ClientUI::new();

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                log_err_client!("Échec de la lecture du thème principal.\nErreur : {}", e);
            }
            audio.stop_main_theme();
        }

        // ui_test(renderer);
    }

    fn update(&mut self, frame: &EngineFrameData, data: &mut GameFrameData, renderer: &mut Renderer) {
        // UPDATE DELAY
        self.delay_s += frame.dt;

        if self.delay_s < DT_CAP {
            spin_sleep::sleep(Duration::from_micros(((DT_CAP - self.delay_s) * 1_000_000.0) as u64));
        }

        self.delay_s -= DT_CAP;

        self.update_debug_commands(&mut renderer.render_manager.world_buffer);

        self.update_physics(frame);

        let (network_commands, mut mesh_request) = self.update_logic(frame, &mut renderer.render_manager.world_buffer);

        self.update_network(network_commands);

        let item_manager = self.world.get_item_manager();
        self.ui.update(
            &self.player.state.inventory,
            self.player.state.selected_slot,
            renderer,
            &item_manager.read().unwrap(),
        );
        let mesh_manager = &mut renderer.render_manager.world_buffer;

        // La position du joueur sert à ordonner les requêtes de maillage par
        // priorité (les chunks les plus proches sont maillés en premier).
        self.world_mesh.set_player_chunk(self.player.get_cpos().into());

        let mut responses = self.world_mesh.update(mesh_manager, &mut mesh_request);
        self.world.listen(&mut responses);

        self.update_rendering(data, renderer);
    }

    fn on_mouse_move(&mut self, dx: f64, dy: f64) {
        self.inputs.set_mouse_delta((dx, dy));
    }

    fn on_mouse_button(&mut self, button: MouseButton, is_pressed: bool) {
        self.inputs.set_mouse_button_press(button, is_pressed);
    }

    fn on_mouse_wheel(&mut self, delta: f32) {
        self.inputs.set_mouse_wheel(delta);
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

#[allow(unused)]
fn ui_test(renderer: &mut Renderer) {
    // UI
    // In 5 steps.

    // 1. Widget tree
    // Build everything you want with it.
    // let test_panel: Panel = {
    //     let transform = WidgetTransform::new(8, 8, 160, 130);
    //     let color = 0xFFCCAAAA;
    //     let child = None;
    //     Panel::new(transform, color, child)
    // };
    let test_panel: TexturedPanel = {
        let transform = WidgetTransform::new(8, 8, 160, 130);
        let texture = 0;
        let child = None;
        TexturedPanel::new(transform, texture, child)
    };

    let mut draw_commands = Vec::new();

    // 2. Draw call
    // At the top of the tree, call root.draw and give it an empty
    // Vec of DrawCommand (it will call .draw() recursively).
    test_panel.draw(&mut draw_commands);

    // 3. Translation
    // When commands are ready, we need to translate them
    // into vertices to be compatible with the shader.
    let vertices = UiTranslator::translate(draw_commands, &renderer.texture_manager);

    // 4. Compilation
    // Transform our UiVertices into raw bytes.
    let bytes = UiCompiler::compile(vertices);

    // 5. Submit the bytes to the GPU, and voila.
    renderer.ui_renderer.update_vertices(&bytes);
}
