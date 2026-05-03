use std::{net::Ipv4Addr, thread::sleep, time::Duration};

use cgmath::{dot, EuclideanSpace, Matrix4, Vector3};

use crate::{
    common::geometry::{plane::Plane, vertex::generate_cube},
    engine::{
        audio::GameAudioManager,
        core::{
            application::AppState,
            frame::{EngineFrameData, GameFrameData},
        },
        render::{mesh::manager::DataEntry, render::{RenderOptions, Renderer}},
    },
    game::{
        network::NetworkManager,
        player::{
            controllers::free::{FreeCameraController, FreePlayerController},
            player::Player,
            remote_players::RemotePlayersManager,
        },
        render::meshing::world::WorldMesh,
        systems::inputs::InputState,
        world::world::World,
    },
};
use shared::{log_client, log_err_client, world::data::chunk::CHUNK_SIZE_F};
use winit::keyboard::KeyCode;

const FPS_CAP: u32 = 60;
const DT_CAP: f32 = 1.0 / (FPS_CAP as f32);
const PING_INTERVAL: Duration = Duration::from_secs(10);

pub struct GameState {
    pub world: World,
    pub world_mesh: WorldMesh,
    pub player: Player,
    pub remote_players: RemotePlayersManager,
    // pub camera: Camera,
    // pub camera_controller: CameraController,
    pub delay_ms: f32,
    pub network: Option<NetworkManager>,
    inputs: InputState,
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
            player: Player::new(Box::new(FreeCameraController::new(1.0)), Box::new(FreePlayerController::new(16.0))),
            world: World::new(server_seed),
            world_mesh: WorldMesh::new(),
            remote_players: RemotePlayersManager::new(),
            inputs: InputState::new(),
            delay_ms: 0.0,
            network: Some(network),
        }
    }
}

impl AppState for GameState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        self.world.update(&mut renderer.render_manager, &mut self.world_mesh, &self.player);
        self.world_mesh.update(renderer, &self.world, &self.player);

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                log_err_client!("Échec de la lecture du thème principal.\nErreur : {}", e);
            }
            audio.stop_main_theme();
        }
    }

    fn update(&mut self, frame: &EngineFrameData, render_options: &RenderOptions, data: &mut GameFrameData, renderer: &mut Renderer) {
        // UPDATE DELAY
        self.delay_ms += frame.dt;

        if self.delay_ms < DT_CAP {
            sleep(Duration::from_micros(((DT_CAP - self.delay_ms) * 1_000_000.0) as u64));
        }

        self.delay_ms -= DT_CAP;

        // LOGIC
        self.player.update(frame.dt, &mut self.inputs);
        self.world.update(&mut renderer.render_manager, &mut self.world_mesh, &self.player);

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
                    let (rx, ry) = self.player.camera.get_rotation();
                    if let Err(e) = net.send_position(pos.x, pos.y, pos.z, rx, ry) {
                        log_err_client!("Échec de l'envoi de la position.\nErreur : {}", e);
                    } else {
                        // log_client!("Position envoyée: ({}, {}, {})", pos.x, pos.y, pos.z);
                    }
                }
                // Réception des positions des autres joueurs
                if let Ok(Some(packet)) = net.receive_packet() {
                    use shared::network::messages::ContenuPaquet;
                    match packet.contenu {
                        ContenuPaquet::MultiplePlayerTransformation { data } => {
                            if let Some(my_id) = net.player_id() {
                                self.remote_players.update(data, my_id);
                            }
                        }
                        _ => {}
                    }
                }
            }            
        }

        // MESHING
        self.world_mesh.update(renderer, &self.world, &self.player);

        // RENDER
        {
            let view_proj = self.player.camera.get_view_proj();
            data.camera.update_view_proj(view_proj);

            self.player.camera.aspect = render_options.aspect;
            let cam_position = self.player.camera.eye.to_vec();
            let cam_forward = self.player.camera.forward();
            let cam_frustum = extract_camera_frustum_planes(view_proj);

            let chunks_to_render = self.player.get_rendered_chunk_keys();

            for (key, mesh) in self.world_mesh.meshes.iter() {
                if mesh.id.is_none() || !chunks_to_render.contains(key) {
                    continue;
                }

                let chunk_vector = Vector3::new(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F);
                let min = Vector3::new((key.0) as f32, (key.1) as f32, (key.2) as f32) * CHUNK_SIZE_F;
                let max = min + chunk_vector;

                // First, we check simply if the chunk to render is behind the camera.
                // Second, we check if the chunk is within the field of view of the camera.
                // If any of the above is true, we do not render the chunk.
                // We do the frustum check after the first one because it is more expansive,
                // and the first one would already eliminate ~50% of the chunks very quickly.
                if is_chunk_behind_camera(&min, &max, &cam_forward, &cam_position) || !is_chunk_in_camera_frustum(&min, &max, &cam_frustum) {
                    continue;
                }

                data.visible_meshes.push(mesh.id.unwrap());
            }

            // Update renderer with remote player positions
            for p in self.remote_players.get_all_mut().iter_mut() {
                let player_data = generate_cube(p.position.0, p.position.1, p.position.2);
                let raw_data = bytemuck::cast_slice(&player_data);
                if let Some(mesh_id) = p.mesh_id {
                    renderer.render_manager.mesh_manager.update_data(
                        &renderer.gpu_context.device,
                        &renderer.gpu_context.queue,
                        &mut renderer.frame_encoder.as_mut().unwrap(),
                        DataEntry::new(mesh_id, raw_data)
                    );
                }
                else {
                    p.mesh_id = renderer.render_manager.mesh_manager.add_data(
                        &renderer.gpu_context.device,
                        &renderer.gpu_context.queue,
                        &mut renderer.frame_encoder.as_mut().unwrap(),
                        raw_data
                    );
                }
                data.visible_meshes.push(p.mesh_id.unwrap());
            }
        }
    }

    // Will be used later on for physics and game's systems' logic
    fn fixed_update(&mut self, _frame: &EngineFrameData, _render_options: &RenderOptions, _data: &mut GameFrameData) {}

    fn on_mouse_move(&mut self, dx: f64, dy: f64) {
        self.inputs.set_mouse_delta((dx, dy));
    }

    fn on_key(&mut self, code: KeyCode, is_pressed: bool) {
        self.inputs.set_key_press(code, is_pressed);
    }
}

fn is_chunk_behind_camera(min: &Vector3<f32>, max: &Vector3<f32>, cam_forward: &Vector3<f32>, cam_eye: &Vector3<f32>) -> bool {
    let center = min + (max - min) * 0.5;
    let extent = (max - min) * 0.5;

    let radius = extent.x * cam_forward.x.abs() + extent.y * cam_forward.y.abs() + extent.z * cam_forward.z.abs();

    let distance = dot(*cam_forward, center - *cam_eye);

    distance + radius < 0.0
}

fn extract_camera_frustum_planes(m: Matrix4<f32>) -> [Plane; 6] {
    [
        Plane {
            normal: Vector3::new(m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0]),
            d: m[3][3] + m[3][0],
        }, // left
        Plane {
            normal: Vector3::new(m[0][3] - m[0][0], m[1][3] - m[1][0], m[2][3] - m[2][0]),
            d: m[3][3] - m[3][0],
        }, // right
        Plane {
            normal: Vector3::new(m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1]),
            d: m[3][3] + m[3][1],
        }, // bottom
        Plane {
            normal: Vector3::new(m[0][3] - m[0][1], m[1][3] - m[1][1], m[2][3] - m[2][1]),
            d: m[3][3] - m[3][1],
        }, // top
        Plane {
            normal: Vector3::new(m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2]),
            d: m[3][3] + m[3][2],
        }, // near
        Plane {
            normal: Vector3::new(m[0][3] - m[0][2], m[1][3] - m[1][2], m[2][3] - m[2][2]),
            d: m[3][3] - m[3][2],
        }, // far
    ]
    .map(|p| p.normalize())
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
