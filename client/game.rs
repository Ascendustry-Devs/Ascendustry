use crate::{
    api::texture_loader::TextureLoader,
    network::NetworkManager,
    player::{
        controllers::{spectator::FreeCameraController, walk::WalkPlayerController},
        remote_players::RemotePlayersManager,
        Player,
    },
    render::{meshing::world::WorldMesh, renderer::GameRenderer},
    systems::texture_registry::TextureRegistry,
    ui::{
        adapters::{hotbar::HotbarAdapter, inventory::InventoryAdapter, UiAdapter},
        context::UiContext,
        layers::{hotbar::HotbarLayer, inventory::InventoryLayer},
        ClientUI,
    },
    world::{mesh_request::MeshRequestMessage, ClientWorld},
};
use engine::{
    audio::GameAudioManager,
    core::{
        application::AppState,
        frame::{EngineFrameData, GameFrameData},
        inputs::InputState,
    },
    gpu::allocator::gpu_allocator::GpuAllocator,
    render::Renderer,
};
use game::world::World;
use network::messages::{new_save_request_paquet, ChunkData, ContenuPaquet, Paquet};
use project_core::log_err_client;
use std::{
    mem::take,
    process::exit,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::time::Instant;
use winit::keyboard::KeyCode;

// TODO: bug de flickering/scintillements dans certains angles de caméra : voir le calcul des meshs à rendre pour la frame
// TODO: hotbar bugguée : lorsqu'on prend 2 items, puis qu'on fini le stack du premier, on perd aussi le deuxième

const FPS_CAP: u32 = 0; // u32::MAX;
const DT_CAP: f32 = {
    if FPS_CAP == 0 {
        0.0
    } else {
        1.0 / (FPS_CAP as f32 + 0.25)
    }
};
const PING_INTERVAL: Duration = Duration::from_secs(10);

pub struct GameState {
    pub world: ClientWorld,
    pub world_mesh: WorldMesh,
    pub player: Player,
    pub remote_players: RemotePlayersManager,
    pub delay_s: f32,
    pub network: Option<NetworkManager>,
    pub ui: ClientUI,
    textures_registry: Option<Arc<TextureRegistry>>,
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
            world: ClientWorld::new(server_seed),
            world_mesh: WorldMesh::new(),
            remote_players: RemotePlayersManager::new(),
            inputs: InputState::new(),
            delay_s: 0.0,
            network: Some(network),
            ui: Default::default(),
            textures_registry: None,
            last_save_request: Instant::now(),
        }
    }

    #[inline(never)]
    fn update_debug_commands(&mut self, alloc: &Arc<RwLock<GpuAllocator>>) {
        // MESH MEMORY OVERVIEW (CPU & GPU)
        if self.inputs.take_key_pressed(KeyCode::Semicolon) {
            println!("==== Memory Tracker ====");
            println!("| World |");
            self.world.print_memory();
            println!("| Meshs |");
            println!("CPU:");
            self.world_mesh.print_memory();
            println!("-----------------------------");
            println!("GPU:");
            alloc.read().unwrap().force_print_debug_infos();
            println!("=============================");
        }
        // MESH MEMORY DUMP (CPU)
        if self.inputs.is_key_pressed(KeyCode::ControlLeft) && self.inputs.is_key_pressed(KeyCode::Semicolon) {
            self.inputs.take_key_pressed(KeyCode::ControlLeft);
            self.inputs.take_key_pressed(KeyCode::Semicolon);
            let alloc = alloc.read().unwrap();
            let meshes = &self.world_mesh.meshes;
            println!("==== Mesh CPU Memory Dump ====");
            for (pos, mesh) in meshes.iter() {
                println!("- Chunk {}", pos);
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
            println!("==== Chunk {} ====", cpos);
            if let Some((state, dirty)) = self.world.chunk_infos_at(cpos) {
                println!("- Data (CPU):\n  └─ {:?}", state);
                if dirty {
                    println!("  └─ Dirty")
                }
            }
            if let Some((id, dirty)) = self.world_mesh.mesh_infos_at(cpos) {
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
            project_core::log_client!("CLIENT WORLD SEED: {}", self.world.seed());
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
                        project_core::log_err_client!("Failed to switch gamemode.\nError: {}", err);
                    }
                }
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
    ) -> (Vec<Paquet>, MeshRequestMessage) {
        let network_commands = self.player.update(frame.dt, &mut self.world, &mut self.inputs);
        let mesh_request = self.world.update(mesh_manager, &mut self.world_mesh, &self.player);
        let mesh_request = take(mesh_request);
        (network_commands, mesh_request)
    }

    #[inline(never)]
    fn update_network(&mut self, network_commands: Vec<Paquet>) {
        {
            let net = self.network.as_mut().expect("NetworkManager: uninitialized.");
            if !net.is_connected() {
                project_core::log_client!("Déconnecté du serveur. Arrêt du client.");
                exit(0);
            }
            // Envoi un ping si aucun échange n'a eu lieu depuis PING_INTERVAL
            if net.get_last_communication().elapsed() >= PING_INTERVAL {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if let Err(e) = net.send_ping(timestamp) {
                    project_core::log_err_client!("Échec de l'envoi du ping.\nErreur : {}", e);
                } else {
                    project_core::log_client!("Ping envoyé !");
                }
            }
        }

        self.apply_received_packets();

        let net = self.network.as_mut().expect("NetworkManager: uninitialized.");

        // Envoi la position et rotation du joueur au serveur si elles ont changés
        if self.player.has_moved() {
            let position = self.player.get_pos();
            let rotation = self.player.state.camera.get_rotation();
            if let Err(e) = net.send_position(*position, *rotation) {
                project_core::log_err_client!("Échec de l'envoi de la position.\nErreur : {}", e);
            }
            self.player.state.reset_moved();
        }

        for command in network_commands {
            let result = net.send_packet(command);
            match result {
                Ok(_) => {}
                Err(err) => {
                    project_core::log_err_client!("Failed to send command packet.\nError: {}", err);
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
                        project_core::log_err_client!("Failed to send save request packet.\nError: {}", err);
                    }
                }
            }
        }
        // Nettoyer les joueurs distants qui n'ont pas envoyé de mise à jour depuis 30s
        self.remote_players.cleanup_stale(Duration::from_secs(30));
    }

    fn apply_received_packets(&mut self) {
        let net = self.network.as_mut().expect("NetworkManager: uninitialized");
        if let Ok(Some(packet)) = net.receive_packet() {
            use ContenuPaquet;
            match packet.contenu {
                // Réception des positions des autres joueurs
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
                    project_core::log_client!("Paquet ContenuPaquet::DonneesMonde reçu !");
                    for ChunkData { position, data } in chunks {
                        self.world.apply_remote_chunk(&position, &data);
                    }
                }
                ContenuPaquet::SetBlock { position, block } => {
                    if let Some(chunk) = self.world.set_block(&position, block).err() {
                        log_err_client!("Couldn't place block {:?} at {}, in chunk {}", block, position, chunk);
                    }
                }
                ContenuPaquet::Kick { reason } => {
                    project_core::log_client!("Kicked by server: {}", reason);
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
    }

    #[inline(always)]
    fn update_rendering(&mut self, data: &mut GameFrameData, renderer: &mut Renderer) {
        self.player.state.camera.aspect.update(renderer.render_options.aspect);
        GameRenderer::render(self, data, renderer);
    }
}

impl AppState for GameState {
    fn inputs(&mut self) -> &mut InputState {
        &mut self.inputs
    }

    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        self.apply_received_packets();
        let mut textures_registry = TextureRegistry::default();
        let mut tex_loader = TextureLoader::new(&mut renderer.texture_manager, &mut textures_registry);
        let meshin = self.world.init(&mut tex_loader, &self.player);
        self.textures_registry = Some(Arc::new(textures_registry));
        let alloc = Arc::clone(&renderer.render_manager.world_buffer);
        self.world_mesh.init(alloc, meshin);

        let frame = EngineFrameData::default();

        let ctx = UiContext::new(
            &self.world,
            &self.player,
            &self.remote_players,
            self.textures_registry.as_ref().unwrap().as_ref(),
            &frame,
        );

        let hotbar = HotbarLayer {
            cols: 8,
            spacing: 2,
            view: HotbarAdapter::adapt((&self.player.state.inventory, self.player.state.selected_slot), &ctx),
        };

        let inventory = InventoryLayer {
            cols: 8,
            rows: 4,
            spacing: 4,
            view: InventoryAdapter::adapt((&self.player.state.inventory, self.player.state.selected_slot), &ctx),
        };

        self.ui = ClientUI::new();
        self.ui.push_layer(Box::new(hotbar));
        self.ui.push_layer(Box::new(inventory));

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                project_core::log_err_client!("Échec de la lecture du thème principal.\nErreur : {}", e);
            }
            audio.stop_main_theme();
        }

        // ui_test(renderer);
    }

    fn update(&mut self, frame: &EngineFrameData, data: &mut GameFrameData, renderer: &mut Renderer) {
        // UPDATE DELAY
        self.delay_s += frame.dt;

        // println!(
        //     "frame took {:7.2}µs ({:3.1}% of budget)",
        //     self.delay_s * 1_000_000.0,
        //     (self.delay_s * 100.0) / DT_CAP
        // );
        if self.delay_s < DT_CAP {
            spin_sleep::sleep(Duration::from_micros(((DT_CAP - self.delay_s) * 1_000_000.0) as u64));
        }

        self.delay_s -= DT_CAP;

        self.update_debug_commands(&renderer.render_manager.world_buffer);

        self.update_physics(frame);

        // Mettre à jour la position du joueur pour le tri des maillages AVANT
        // world.update / world_mesh.update afin d'avoir la bonne priorité dès cette frame.
        self.world_mesh.set_player_chunk(self.player.get_cpos());

        let (network_commands, mut mesh_request) = self.update_logic(frame, &mut renderer.render_manager.world_buffer);

        self.update_network(network_commands);

        // let item_manager = self.world.get_item_manager();

        let ctx = UiContext::new(
            &self.world,
            &self.player,
            &self.remote_players,
            self.textures_registry.as_ref().unwrap().as_ref(),
            frame,
        );

        self.ui.update(&ctx);

        let surface_config = &renderer.gpu_context.config;
        let available_space = (surface_config.width, surface_config.height);
        self.ui
            .submit(&available_space, &mut renderer.ui_renderer, &renderer.texture_manager);
        let mesh_manager = &mut renderer.render_manager.world_buffer;

        let textures_registry = Arc::clone(self.textures_registry.as_ref().unwrap());
        let mut responses = self.world_mesh.update(mesh_manager, &mut mesh_request, textures_registry);
        self.world.listen(&mut responses);

        self.update_rendering(data, renderer);
    }

    fn dispose(&mut self, alloc: &mut Arc<RwLock<GpuAllocator>>) {
        // TODO: faire fonctionner -> // Network dispose (disconnection, memory release...), if any.
        // if let Some(net) = self.network.as_mut() {

        // }
        self.world.dispose();
        self.world_mesh.dispose(alloc);
    }
}
