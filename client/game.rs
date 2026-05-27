use crate::api::texture_loader::TextureLoader;
use crate::network::NetworkManager;
use crate::player::controllers::spectator::FreeCameraController;
use crate::player::controllers::walk::WalkPlayerController;
use crate::player::player::Player;
use crate::player::remote_players::RemotePlayersManager;
use crate::render::meshing::world::WorldMesh;
use crate::systems::inputs::InputState;
use crate::world::world::World;
use bytemuck::cast_slice;
use cgmath::EuclideanSpace;
use cgmath::{dot, Vector3};
use engine::audio::GameAudioManager;
use engine::core::application::AppState;
use engine::core::frame::EngineFrameData;
use engine::core::frame::GameFrameData;
use engine::geometry::vertex::generate_cube;
use engine::render::render::Renderer;
use game::constants::CHUNK_VECTOR;
use game::world::data::chunk::CHUNK_SIZE_F;
use network::messages::new_save_request_paquet;
use network::messages::ContenuPaquet;
use satiscore::geometry::plane::Plane;
use satiscore::{log_client, log_err_client};
use std::time::Duration;
use tokio::time::Instant;
use winit::keyboard::KeyCode;

const FPS_CAP: u32 = 0;
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
    pub fn new(addr: String) -> Self {
        let mut network = NetworkManager::new();

        network.connect(&addr);
        let server_seed = network
            .perform_handshake("Player")
            .ok()
            .and_then(|_| network.get_server_seed())
            .expect("La seed n'existe pas ou est vide (serveur non lancé ? connexion échouée ? mauvaise adresse IP ?)");

        Self {
            player: Player::new(Box::new(FreeCameraController::new(0.00390625)), Box::new(WalkPlayerController)),
            world: World::new(server_seed),
            world_mesh: WorldMesh::new(),
            remote_players: RemotePlayersManager::new(),
            inputs: InputState::new(),
            delay_s: 0.0,
            network: Some(network),
            last_save_request: Instant::now(),
        }
    }
}

impl AppState for GameState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        let mut tex_loader = TextureLoader::new(&mut renderer.texture_manager);
        self.world.init(&mut tex_loader, &self.player);
        self.world_mesh.init(&mut self.world);

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                log_err_client!("Échec de la lecture du thème principal.\nErreur : {}", e);
            }
            audio.stop_main_theme();
        }
    }

    fn update(&mut self, frame: &EngineFrameData, data: &mut GameFrameData, renderer: &mut Renderer) {
        // UPDATE DELAY
        self.delay_s += frame.dt;

        if self.delay_s < DT_CAP {
            spin_sleep::sleep(Duration::from_micros(((DT_CAP - self.delay_s) * 1_000_000.0) as u64));
        }

        self.delay_s -= DT_CAP;

        // Commande debug (touches)
        self.update_debug_commands();

        // PHYSICS
        self.player
            .physics_update(frame.dt, &mut self.inputs, &self.world, self.player.state.game_mode.clone());

        // LOGIC
        let network_commands = self.player.update(frame.dt, &mut self.world, &mut self.inputs);
        let mesh_manager = &mut renderer.render_manager.mesh_manager;
        self.world.update(mesh_manager, &mut self.world_mesh, &self.player);

        // NETWORK
        {
            let net = self.network.as_mut().expect("NetworkManager: uninitialized.");
            if net.is_connected() {
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
                            if let Some(my_id) = net.player_id() {
                                self.remote_players.update(data, my_id);
                            }
                        }
                        ContenuPaquet::GuardCorrection { data } => {
                            if let Some(my_id) = net.player_id() {
                                for t in &data {
                                    if t.player_id == my_id {
                                        self.player.state.set_position_and_rotation(t.position, t.rotation);
                                    }
                                }
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
        }

        // MESHING
        self.world_mesh.update(mesh_manager, &mut self.world);

        // RENDER
        {
            let view_proj = self.player.state.camera.get_view_proj();
            let (cam_x, cam_y, cam_z) = {
                let pos = self.player.state.camera.eye();
                (pos.x, pos.y, pos.z)
            };
            data.camera.update(cam_x, cam_y, cam_z, (*view_proj).into());

            self.player.state.camera.aspect.update(renderer.render_options.aspect);
            let cam_position = (*self.player.state.camera.eye()).to_vec();
            let cam_forward = self.player.state.camera.forward();
            let cam_frustum = self.player.state.camera.get_frustum_planes();

            let chunks_to_render = self.player.get_rendered_chunk_keys_set();

            for (key, mesh) in self.world_mesh.meshes.iter() {
                if mesh.id.is_none() || !chunks_to_render.contains(key) {
                    continue;
                }

                let min = Vector3::new((key.0) as f32, (key.1) as f32, (key.2) as f32) * CHUNK_SIZE_F;
                let max = min + CHUNK_VECTOR;

                // First, we check simply if the chunk to render is behind the camera.
                if is_chunk_behind_camera(&min, &max, &cam_forward, &cam_position) {
                    continue;
                }

                // Second, we check if the chunk is within the field of view of the camera.
                if !is_chunk_in_camera_frustum(&min, &max, &cam_frustum) {
                    continue;
                }

                // If any of the above is true, we do not render the chunk.
                // We do the frustum check lately because it is more expansive,
                // on top of this, the first check would already eliminate ~50% of the candidates.

                data.visible_meshes.insert(mesh.id.unwrap());
            }

            // Update renderer with remote player positions
            for p in self.remote_players.get_all_mut().iter_mut() {
                if let Some(new_pos) = p.position.change() {
                    let player_data = generate_cube(new_pos.0, new_pos.1, new_pos.2);
                    let raw_data = cast_slice(&player_data);
                    if let Some(mesh_id) = p.mesh_id {
                        if let Some(update_err) = mesh_manager.update(mesh_id, raw_data).err() {
                            println!("Failed to update mesh id {}.\nError: {}", mesh_id, update_err);
                        };
                    } else {
                        p.mesh_id = mesh_manager.add(raw_data).ok();
                    }
                }
                data.visible_meshes.insert(p.mesh_id.unwrap());
            }
        }
    }

    fn on_mouse_move(&mut self, dx: f64, dy: f64) {
        self.inputs.set_mouse_delta((dx, dy));
    }

    fn on_key(&mut self, code: KeyCode, is_pressed: bool) {
        self.inputs.set_key_press(code, is_pressed);
    }

    fn dispose(&mut self) {
        // TODO: faire fonctionner -> // Network dispose (disconnection, memory release...), if any.
        // if let Some(net) = self.network.as_mut() {

        // }

        self.world.dispose();
        self.world_mesh.dispose();
    }
}

impl GameState {
    fn update_debug_commands(&mut self) {
        // CURRENT CHUNK DEBUG
        if self.inputs.take_key_pressed(KeyCode::KeyC) {
            let cpos = self.player.state.cpos.current();
            let key = (cpos[0], cpos[1], cpos[2]);
            println!("---------\nDEBUG: Chunk x={} y={} z={}\n---------", key.0, key.1, key.2);
            if let Some((state, dirty)) = self.world.chunk_infos_at(&key) {
                println!("General:\n- State: {}\n- Is dirty?: {}", state, dirty);
            }
            if let Some((id, dirty)) = self.world_mesh.mesh_infos_at(&key) {
                let id = match id {
                    Some(id) => &id.to_string(),
                    None => "None",
                };
                println!("Mesh:\n- Id: {}\n- Is dirty?: {}", id, dirty);
            };
            println!("---------")
        }

        // SWITCH GAMEMODE
        if self.inputs.take_key_pressed(KeyCode::KeyG) {
            println!("Switch gamemode");
            self.player.state.switch_player_game_mode();
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
}

fn is_chunk_behind_camera(min: &Vector3<f32>, max: &Vector3<f32>, cam_forward: &Vector3<f32>, cam_eye: &Vector3<f32>) -> bool {
    let extent = (max - min) * 0.5;
    let center = min + extent;

    let radius = extent.x * cam_forward.x.abs() + extent.y * cam_forward.y.abs() + extent.z * cam_forward.z.abs();

    let distance = dot(*cam_forward, center - *cam_eye);

    distance + radius < 0.0
}

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
