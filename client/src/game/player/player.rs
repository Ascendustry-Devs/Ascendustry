use crate::common::utils::updatable::Updatable;
use crate::game::player::controllers::spectator::SpectatorPlayerController;
use crate::game::player::controllers::walk::WalkPlayerController;
use crate::game::{
    physics::{body::PhysicsBody, collision::resolve_collision},
    player::camera::Camera,
    player::controllers::{CameraController, PlayerController},
    systems::inputs::InputState,
    world::world::World,
};
use cgmath::{num_traits::ToPrimitive, Point3};
use shared::network::messages::{PlayerGameMode, Position, Rotation};
use shared::world::constants::{
    HORIZONTAL_RENDER_DISTANCE, HORIZONTAL_SIMULATION_DISTANCE, SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z,
    VERTICAL_RENDER_DISTANCE, VERTICAL_SIMULATION_DISTANCE,
};
use shared::world::data::chunk::{CHUNK_SIZE, CHUNK_SIZE_F};
use shared::*;

/// État pur du joueur : position, caméra, contrôleurs, distances de rendu.
/// Séparé de `Player` pour permettre l'ajout d'un corps physique sans tout casser.
pub struct PlayerState {
    uuid: i32,
    pub pos: Updatable<cgmath::Point3<f32>>,
    pub cpos: Updatable<cgmath::Point3<i32>>,
    pub game_mode: PlayerGameMode,
    pub horizontal_render_distance: u16,
    pub vertical_render_distance: u16,
    pub horizontal_simulation_distance: u16,
    pub vertical_simulation_distance: u16,
    pub camera_controller: Box<dyn CameraController>,
    pub player_controller: Box<dyn PlayerController>,
    pub camera: Camera,
}

impl PlayerState {
    pub fn new(
        camera_controller: Box<dyn CameraController>,
        player_controller: Box<dyn PlayerController>,
        spawn_pos: Point3<f32>,
    ) -> PlayerState {
        PlayerState {
            game_mode: PlayerGameMode::Survival,
            uuid: -1,
            pos: Updatable::new(spawn_pos),
            cpos: Updatable::new(spawn_pos.map(|coord| coord.div_euclid(CHUNK_SIZE as f32).floor() as i32)),
            horizontal_render_distance: HORIZONTAL_RENDER_DISTANCE,
            vertical_render_distance: VERTICAL_RENDER_DISTANCE,
            horizontal_simulation_distance: HORIZONTAL_SIMULATION_DISTANCE,
            vertical_simulation_distance: VERTICAL_SIMULATION_DISTANCE,
            camera_controller,
            player_controller,
            camera: Camera::new(spawn_pos, 1.0),
        }
    }

    /// Met à jour la caméra (yaw/pitch).
    /// La position et `cpos` sont mis à jour dans `physics_update()` (timestep fixe).
    pub fn update(&mut self, dt: f32, inputs: &mut InputState) {
        let pos = self.get_pos();
        self.camera_controller.update(dt, inputs, &mut self.camera, &pos);
    }

    pub fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.horizontal_render_distance = horizontal;
        self.vertical_render_distance = vertical;
    }

    pub fn set_player_controller(&mut self, player_controller: Box<dyn PlayerController>) {
        self.player_controller = player_controller;
    }

    pub fn switch_player_game_mode(&mut self) {
        match self.game_mode {
            PlayerGameMode::Spectator => {
                self.set_player_controller(Box::new(WalkPlayerController));
                self.game_mode = PlayerGameMode::Survival;
            }
            PlayerGameMode::Survival => {
                self.set_player_controller(Box::new(SpectatorPlayerController::new(15.0)));
                self.game_mode = PlayerGameMode::Spectator;
            }
        }
    }

    pub fn get_pos(&self) -> cgmath::Point3<f32> {
        self.pos.current().clone()
    }

    pub fn get_cpos(&self) -> cgmath::Point3<i32> {
        self.cpos.current().clone()
    }

    pub fn has_moved(&self) -> bool {
        self.pos.has_changed()
    }

    /// Réinitialise le flag `has_moved` (appelé après envoi réseau).
    pub fn reset_moved(&mut self) {
        self.pos.update(self.pos.current().clone());
    }

    pub fn set_pos(&mut self, pos: cgmath::Point3<f32>) {
        self.pos.update(pos);
        self.cpos
            .update(self.pos.current().map(|coord| coord.div_euclid(CHUNK_SIZE as f32).floor() as i32));
    }

    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        log_client!(
            "Joueur {}: téléportation de {:?} à {:?}",
            self.uuid,
            self.get_pos(),
            Point3 { x: x, y: y, z: z }
        );
        self.set_pos(Point3 { x: x, y: y, z: z });
    }

    pub fn set_position_and_rotation(&mut self, pos: Position, rot: Rotation) {
        self.set_pos(Point3::new(pos.x, pos.y, pos.z));
        self.camera.yaw = rot.x;
        self.camera.pitch = rot.y;
    }

    pub fn break_block_at(_block_pos: Point3<f32>) {}

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à simuler autour du joueur.
    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        let halfed_hrd = self.horizontal_simulation_distance.to_f32().unwrap().div_euclid(2.0);
        let halfed_vrd = self.vertical_simulation_distance.to_f32().unwrap().div_euclid(2.0);

        let cx = self.pos.current().x.div_euclid(CHUNK_SIZE as f32);
        let cy = self.pos.current().y.div_euclid(CHUNK_SIZE as f32);
        let cz = self.pos.current().z.div_euclid(CHUNK_SIZE as f32);

        let min_cx = (cx - halfed_hrd).floor().to_i32().unwrap();
        let max_cx = (cx + halfed_hrd).floor().to_i32().unwrap();
        let min_cy = (cy - halfed_vrd).floor().to_i32().unwrap();
        let max_cy = (cy + halfed_vrd).floor().to_i32().unwrap();
        let min_cz = (cz - halfed_hrd).floor().to_i32().unwrap();
        let max_cz = (cz + halfed_hrd).floor().to_i32().unwrap();

        return [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz];
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à afficher autour du joueur.
    pub fn get_rendered_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_rendered_chunk_range();

        let mut keys: Vec<(i32, i32, i32)> = Vec::new();

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    keys.push((x, y, z));
                }
            }
        }

        return keys;
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à simuler autour du joueur.
    pub fn get_simulation_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_simulation_chunk_range();

        let mut keys: Vec<(i32, i32, i32)> = Vec::new();

        for x in min_cx..=max_cx {
            for y in min_cy..=max_cy {
                for z in min_cz..=max_cz {
                    keys.push((x, y, z));
                }
            }
        }

        return keys;
    }

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à rendre autour du joueur.
    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        let halfed_hrd = self.horizontal_render_distance.to_f32().unwrap().div_euclid(2.0);
        let halfed_vrd = self.vertical_render_distance.to_f32().unwrap().div_euclid(2.0);

        let cx = self.pos.current().x.div_euclid(CHUNK_SIZE as f32);
        let cy = self.pos.current().y.div_euclid(CHUNK_SIZE as f32);
        let cz = self.pos.current().z.div_euclid(CHUNK_SIZE as f32);

        let min_cx = (cx - halfed_hrd).floor().to_i32().unwrap();
        let max_cx = (cx + halfed_hrd).floor().to_i32().unwrap();
        let min_cy = (cy - halfed_vrd).floor().to_i32().unwrap();
        let max_cy = (cy + halfed_vrd).floor().to_i32().unwrap();
        let min_cz = (cz - halfed_hrd).floor().to_i32().unwrap();
        let max_cz = (cz + halfed_hrd).floor().to_i32().unwrap();

        return [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz];
    }

    /// Retourne les infos de rendu : plage de chunks + nombre total de chunks.
    pub fn get_rendered_chunk_data(&self) -> ([i32; 6], u32) {
        let halfed_hrd = self.horizontal_render_distance.to_f32().unwrap().div_euclid(2.0);
        let halfed_vrd = self.vertical_render_distance.to_f32().unwrap().div_euclid(2.0);

        let cx = self.pos.current().x.div_euclid(CHUNK_SIZE as f32);
        let cy = self.pos.current().y.div_euclid(CHUNK_SIZE as f32);
        let cz = self.pos.current().z.div_euclid(CHUNK_SIZE as f32);

        let min_cx = (cx - halfed_hrd).floor().to_i32().unwrap();
        let max_cx = (cx + halfed_hrd).floor().to_i32().unwrap();
        let min_cy = (cy - halfed_vrd).floor().to_i32().unwrap();
        let max_cy = (cy + halfed_vrd).floor().to_i32().unwrap();
        let min_cz = (cz - halfed_hrd).floor().to_i32().unwrap();
        let max_cz = (cz + halfed_hrd).floor().to_i32().unwrap();

        let chunk_number = ((max_cx - min_cx) * (max_cy - min_cy) * (max_cz - min_cz)).to_u32().unwrap_or(1);

        return ([min_cx, max_cx, min_cy, max_cy, min_cz, max_cz], chunk_number);
    }
}

/// Structure principale du joueur local.
/// Contient l'état pur (`PlayerState`) + le corps physique (`PhysicsBody`).
pub struct Player {
    pub state: PlayerState,
    pub physics_body: PhysicsBody,
}

impl Player {
    pub fn new(camera_controller: Box<dyn CameraController>, player_controller: Box<dyn PlayerController>) -> Player {
        let spawn_pos = Point3::new(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        Player {
            state: PlayerState::new(camera_controller, player_controller, spawn_pos),
            physics_body: PhysicsBody::new(spawn_pos, 0.49),
        }
    }

    /// Délègue la mise à jour de la caméra à `PlayerState` (yaw/pitch).
    pub fn update(&mut self, dt: f32, inputs: &mut InputState) {
        self.state.update(dt, inputs);
    }

    /// Met à jour la physique à timestep fixe :
    /// 1. Le contrôleur joueur modifie la vélocité du `PhysicsBody`
    /// 2. `resolve_collision` traduit la vélocité en déplacement et corrige les collisions
    pub fn physics_update(&mut self, dt: f32, inputs: &mut InputState, world: &World, player_game_mode: PlayerGameMode) {
        self.state
            .player_controller
            .update(dt, inputs, &mut self.physics_body, &self.state.camera);
        match player_game_mode {
            PlayerGameMode::Spectator => {
                let pos = self.state.pos.current_mut();
                *pos += self.physics_body.velocity * dt;
            }
            PlayerGameMode::Survival => {
                resolve_collision(world, &mut self.physics_body, dt, self.state.pos.current_mut());
            }
        }

        self.state
            .cpos
            .update(self.state.pos.current().map(|coord| coord.div_euclid(CHUNK_SIZE_F).floor() as i32));
    }

    /// Délègue à `self.state`.
    pub fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.state.set_render_distance(horizontal, vertical);
    }

    pub fn get_pos(&self) -> cgmath::Point3<f32> {
        self.state.get_pos()
    }

    pub fn get_cpos(&self) -> cgmath::Point3<i32> {
        self.state.get_cpos()
    }

    pub fn has_moved(&self) -> bool {
        self.state.has_moved()
    }

    pub fn set_pos(&mut self, pos: cgmath::Point3<f32>) {
        self.state.set_pos(pos);
    }

    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        self.state.teleport(x, y, z);
    }

    pub fn break_block_at(block_pos: Point3<f32>) {
        PlayerState::break_block_at(block_pos);
    }

    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        self.state.get_simulation_chunk_range()
    }

    pub fn get_rendered_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        self.state.get_rendered_chunk_keys()
    }

    pub fn get_simulation_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        self.state.get_simulation_chunk_keys()
    }

    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        self.state.get_rendered_chunk_range()
    }

    pub fn get_rendered_chunk_data(&self) -> ([i32; 6], u32) {
        self.state.get_rendered_chunk_data()
    }
}
