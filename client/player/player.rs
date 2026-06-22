use crate::network::protocol::GameProtocol;
use crate::player::controllers::spectator::SpectatorPlayerController;
use crate::player::controllers::walk::WalkPlayerController;

use crate::player::camera::Camera;
use crate::player::controllers::{CameraController, PlayerController};
use crate::systems::inputs::InputState;
use crate::world::world::World;
use cgmath::Point3;
use game::constants::{
    HORIZONTAL_RENDER_DISTANCE, HORIZONTAL_SIMULATION_DISTANCE, RENDER_DISTANCE_CHUNK_COUNT, SPAWN_POSITION_X,
    SPAWN_POSITION_Y, SPAWN_POSITION_Z, VERTICAL_RENDER_DISTANCE, VERTICAL_SIMULATION_DISTANCE,
};
use game::inventory::{Inventory, ItemData, ItemRules, DEFAULT_INVENTORY_SIZE};
use game::player::PlayerGameMode;
use game::types::{Position, Rotation};
use game::world::data::block::BlockInstance;
use game::world::data::chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_F};
use game::world::raycast::voxel_raycast;
use network::messages::{new_inventory_update_paquet, Paquet};
use physics::{body::PhysicsBody, collision::resolve_collision};
use project_core::log_client;
use project_core::utils::updatable::Updatable;
use rustc_hash::{FxBuildHasher, FxHashSet};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// État pur du joueur : position, caméra, contrôleurs, distances de rendu.
/// Séparé de `Player` pour permettre l'ajout d'un corps physique sans tout casser.
pub struct PlayerState {
    pub player_id: u64,
    pub pos: Updatable<Point3<f32>>,
    pub cpos: Updatable<Point3<i32>>,
    pub chunk_keys: FxHashSet<(i32, i32, i32)>,
    pub game_mode: PlayerGameMode,
    pub inventory: Inventory,
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
        player_id: u64,
        camera_controller: Box<dyn CameraController>,
        player_controller: Box<dyn PlayerController>,
        spawn_pos: Point3<f32>,
    ) -> PlayerState {
        PlayerState {
            player_id,
            game_mode: PlayerGameMode::Survival,
            inventory: Inventory::default(DEFAULT_INVENTORY_SIZE),
            pos: Updatable::new(spawn_pos),
            cpos: Updatable::new(spawn_pos.map(|coord| coord.div_euclid(CHUNK_SIZE as f32).floor() as i32)),
            chunk_keys: FxHashSet::with_capacity_and_hasher(RENDER_DISTANCE_CHUNK_COUNT as usize, FxBuildHasher),
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
    pub fn update(&mut self, dt: f32, world: &mut World, inputs: &mut InputState) -> Vec<Paquet> {
        let pos = self.get_pos();
        self.camera_controller.update(dt, inputs, &mut self.camera, &pos);

        let mut commands = Vec::new();

        if inputs.take_key_pressed(KeyCode::KeyE) {
            log_client!("{}", self.inventory);
        }

        if inputs.take_mouse_button_pressed(MouseButton::Left) {
            self.break_block(world, &mut commands);
        }

        commands
    }

    fn break_block(&mut self, world: &mut World, commands: &mut Vec<Paquet>) {
        let hit = voxel_raycast(self.camera.eye(), &self.camera.forward(), 4.0, |x, y, z| {
            world.get_block_from_xyz(x, y, z).is_solid()
        });
        if let Some(hit) = hit {
            let (x, y, z) = hit.block_pos;
            let block_id = world.get_block_from_xyz(x, y, z).get_block_id();
            let item = world.block_to_item(block_id);
            if let Some(item) = item {
                log_client!("Ajout d'un item dans l'inventaire");
                if let Some(slot) = self.inventory.add_item(ItemData::new(item, None), 1, &ItemRules::default()) {
                    commands.push(new_inventory_update_paquet(self.player_id, vec![slot]));
                }
            }
            let air = BlockInstance::air();
            let success = world.set_block(x, y, z, air);
            if success {
                commands.push(GameProtocol::create_block_modification(x, y, z, air));
            }
        }
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

    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        log_client!(
            "Téléportation du joueur de {:?} à {:?}",
            self.get_pos(),
            Point3 { x: x, y: y, z: z }
        );
        self.set_pos(Point3 { x: x, y: y, z: z });
    }

    pub fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.horizontal_render_distance = horizontal;
        self.vertical_render_distance = vertical;
    }

    pub fn set_player_controller(&mut self, player_controller: Box<dyn PlayerController>) {
        self.player_controller = player_controller;
    }

    pub fn get_pos(&self) -> Point3<f32> {
        self.pos.current().clone()
    }

    pub fn get_cpos(&self) -> Point3<i32> {
        self.cpos.current().clone()
    }

    pub fn has_moved(&self) -> bool {
        self.pos.has_changed()
    }

    /// Réinitialise le flag `has_moved` (appelé après envoi réseau).
    pub fn reset_moved(&mut self) {
        self.pos.update(self.pos.current().clone());
    }

    pub fn set_pos(&mut self, pos: Point3<f32>) {
        self.pos.update(pos);
        self.cpos.update(
            self.pos
                .current()
                .map(|coord| coord.div_euclid(CHUNK_SIZE as f32).floor() as i32),
        );
    }

    pub fn set_rot(&mut self, rot: Rotation) {
        self.camera.set_rotation((rot.x, rot.y));
    }

    pub fn set_position_and_rotation(&mut self, pos: Position, rot: Rotation) {
        self.set_pos(Point3::new(pos.x, pos.y, pos.z));
        self.set_rot(rot);
    }

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à simuler autour du joueur.
    #[inline(always)]
    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        Chunk::get_cube_chunk_range(
            (*self.cpos.current()).into(),
            self.horizontal_simulation_distance,
            self.vertical_simulation_distance,
        )
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à simuler autour du joueur.
    #[inline(always)]
    pub fn get_simulation_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_simulation_chunk_range();
        Chunk::get_cube_chunk_keys(min_cx, max_cx, min_cy, max_cy, min_cz, max_cz)
    }

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à rendre autour du joueur.
    #[inline(always)]
    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        Chunk::get_cube_chunk_range(
            (*self.cpos.current()).into(),
            self.horizontal_render_distance,
            self.vertical_render_distance,
        )
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à afficher autour du joueur.
    #[inline(always)]
    pub fn get_rendered_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_rendered_chunk_range();
        Chunk::get_cube_chunk_keys(min_cx, max_cx, min_cy, max_cy, min_cz, max_cz)
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à afficher autour du joueur.
    ///
    /// Retourne un FxHashSet.
    #[inline(always)]
    pub fn get_rendered_chunk_keys_set(&self) -> FxHashSet<(i32, i32, i32)> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_rendered_chunk_range();
        Chunk::get_cube_chunk_keys_set(min_cx, max_cx, min_cy, max_cy, min_cz, max_cz)
    }
}

/// Structure principale du joueur local.
/// Contient l'état pur (`PlayerState`) + le corps physique (`PhysicsBody`).
pub struct Player {
    pub state: PlayerState,
    pub physics_body: PhysicsBody,
}

impl Player {
    pub fn new(
        player_id: u64,
        camera_controller: Box<dyn CameraController>,
        player_controller: Box<dyn PlayerController>,
    ) -> Player {
        let spawn_pos = Point3::new(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        Player {
            state: PlayerState::new(player_id, camera_controller, player_controller, spawn_pos),
            physics_body: PhysicsBody::new(spawn_pos, 0.49),
        }
    }

    /// Délègue la mise à jour de la caméra à `PlayerState` (yaw/pitch).
    pub fn update(&mut self, dt: f32, world: &mut World, inputs: &mut InputState) -> Vec<Paquet> {
        self.state.update(dt, world, inputs)
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
                *pos += self.physics_body.velocity.current() * dt;
            }
            PlayerGameMode::Survival => {
                resolve_collision(world, &mut self.physics_body, dt, self.state.pos.current_mut());
            }
        }

        self.state.cpos.update(
            self.state
                .pos
                .current()
                .map(|coord| coord.div_euclid(CHUNK_SIZE_F).floor() as i32),
        );
        if self.state.cpos.has_changed() {
            self.state.chunk_keys = self.state.get_rendered_chunk_keys_set();
        }
    }

    /// Délègue à `self.state`.
    pub fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.state.set_render_distance(horizontal, vertical);
    }

    pub fn get_pos(&self) -> Point3<f32> {
        self.state.get_pos()
    }

    pub fn get_cpos(&self) -> Point3<i32> {
        self.state.get_cpos()
    }

    pub fn has_moved(&self) -> bool {
        self.state.has_moved()
    }

    pub fn set_pos(&mut self, pos: Point3<f32>) {
        self.state.set_pos(pos);
    }

    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        self.state.teleport(x, y, z);
    }

    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        self.state.get_simulation_chunk_range()
    }

    pub fn get_rendered_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        self.state.get_rendered_chunk_keys()
    }

    pub fn get_rendered_chunk_keys_set(&self) -> &FxHashSet<(i32, i32, i32)> {
        &self.state.chunk_keys
    }

    pub fn get_simulation_chunk_keys(&self) -> Vec<(i32, i32, i32)> {
        self.state.get_simulation_chunk_keys()
    }

    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        self.state.get_rendered_chunk_range()
    }
}
