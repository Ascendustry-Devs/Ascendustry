use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use cgmath::{dot, EuclideanSpace, Matrix4, Vector3};

use crate::{
    common::geometry::plane::Plane,
    engine::{
        audio::GameAudioManager,
        core::{
            application::AppState,
            frame::{EngineFrameData, GameFrameData},
        },
        render::render::{RenderOptions, Renderer},
    },
    game::{
        network::NetworkManager,
        player::{
            controllers::free::{FreeCameraController, FreePlayerController}, player::Player
        },
        systems::inputs::InputState,
        render::meshing::world::WorldMesh,
        world::world::World,
    },
};
use shared::{
    log_client, log_err_client,
    world::constants::{MAX_CHUNKS_PER_BATCH, MIN_CHUNKS_PER_BATCH},
    world::data::chunk::CHUNK_SIZE_F,
};
use winit::keyboard::KeyCode;

const FPS_CAP: u32 = 1_000_000;
const DT_CAP: f32 = 1.0 / (FPS_CAP as f32);
const PING_INTERVAL: Duration = Duration::from_secs(10);

pub struct GameState {
    pub world: World,
    pub world_mesh: WorldMesh,
    pub player: Player,
    // pub camera: Camera,
    // pub camera_controller: CameraController,
    pub delay_ms: f32,
    pub network: Option<NetworkManager>,
    inputs: InputState,
    last_ping: Instant,
}

impl GameState {
    pub fn new() -> Self {
        let mut network = NetworkManager::new();
        network.connect("127.0.0.1:5000");
        let server_seed = network
            .perform_handshake("Player")
            .ok()
            .and_then(|_| network.get_server_seed())
            .expect("La seed est vide");

        Self {
            player: Player::new(
                Box::new(FreeCameraController::new(1.0)),
                Box::new(FreePlayerController::new(16.0)),
            ),
            world: World::new(server_seed),
            world_mesh: WorldMesh::new(),
            inputs: InputState::new(),
            delay_ms: 0.0,
            network: Some(network),
            last_ping: Instant::now(),
        }
    }
}

impl AppState for GameState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>) {
        self.world.update(&mut renderer.render_manager, &mut self.world_mesh, &self.player);
        self.world_mesh.update(renderer, &self.world, &self.player);

        if let Some(ref mut audio) = audio_manager {
            if let Err(e) = audio.play_main_theme() {
                log_err_client!("Failed to play main theme: {}", e);
            }
            audio.stop_main_theme();
        }
    }

    fn update(&mut self, frame: &EngineFrameData, render_options: &RenderOptions, data: &mut GameFrameData, renderer: &mut Renderer) {
        self.delay_ms += frame.dt;

        if self.delay_ms < DT_CAP {
            sleep(Duration::from_micros(((DT_CAP - self.delay_ms) * 1_000_000.0) as u64));
        }

        self.delay_ms -= DT_CAP;

        // Envoi ping toutes les 10 secondes
        if let Some(ref mut net) = self.network {
            if net.is_connected() && self.last_ping.elapsed() >= PING_INTERVAL {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if let Err(e) = net.send_ping(timestamp) {
                    log_err_client!("Erreur envoi ping: {}", e);
                } else {
                    log_client!("Ping envoye!");
                    self.last_ping = Instant::now();
                }
            }
        }

        // LOGIC
        self.player.update(frame.dt, &mut self.inputs);

        if let Some(ref mut net) = self.network {
            if net.is_connected() && self.player.has_moved() {
                let pos = self.player.get_pos();
                let (rx, ry) = self.player.camera.get_rotation();
                if let Err(e) = net.send_position(pos.x, pos.y, pos.z, rx, ry) {
                    log_err_client!("Erreur envoi position: {}", e);
                }
            }
        }

        self.world.update(&mut renderer.render_manager, &mut self.world_mesh, &self.player);

        let pending_validations = self.world.take_pending_validations();

        // Filtrer pour n'envoyer que les chunks dans la distance de rendu
        let render_range = self.player.get_rendered_chunk_range();
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = render_range;

        let chunks: Vec<_> = pending_validations
            .into_iter()
            .filter(|(cx, cy, cz, _)| *cx >= min_cx && *cx <= max_cx && *cy >= min_cy && *cy <= max_cy && *cz >= min_cz && *cz <= max_cz)
            .map(|(cx, cy, cz, checksum)| shared::network::messages::BatchChunkChecksum {
                x: cx,
                y: cy,
                z: cz,
                checksum,
            })
            .collect();

        if chunks.len() >= MIN_CHUNKS_PER_BATCH && self.network.is_some() {
            for chunk_batch in chunks.chunks(MAX_CHUNKS_PER_BATCH) {
                if let Some(ref mut net) = self.network {
                    let batch: Vec<_> = chunk_batch.to_vec();
                    log_client!("Envoi ChunkValidationBatchRequest avec {} chunks", batch.len());
                    if let Err(e) = net.send_chunk_validation_batch(batch) {
                        log_err_client!("Erreur validation batch: {}", e);
                    }
                }
            }
        }

        self.world_mesh.update(renderer, &self.world, &self.player);

        // RENDER
        let view_proj = self.player.camera.get_view_proj();
        data.camera.update_view_proj(view_proj);

        self.player.camera.aspect = render_options.aspect;
        let cam_position = self.player.camera.eye.to_vec();
        let cam_forward = self.player.camera.forward();
        let cam_frustum = extract_camera_frustum_planes(view_proj);

        let chunks_to_render = self.player.get_rendered_chunk_keys();

        // println!("chunks to render: {:?}", chunks_to_render);
        // println!("world meshes: {:?}", self.world_mesh.meshes.len());

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
