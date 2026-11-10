pub mod camera;
pub mod controllers;
pub mod remote_players;

use crate::network::protocol::GameProtocol;
use crate::player::controllers::spectator::SpectatorPlayerController;
use crate::player::controllers::walk::WalkPlayerController;

use crate::player::camera::Camera;
use crate::player::controllers::{CameraController, PlayerController};
use crate::world::ClientWorld;
use engine::core::inputs::InputState;
use game::constants::{
    HORIZONTAL_RENDER_DISTANCE, HORIZONTAL_SIMULATION_DISTANCE, SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z,
    SPECTATOR_FLY_SPEED, VERTICAL_RENDER_DISTANCE, VERTICAL_SIMULATION_DISTANCE,
};
use game::inventory::{Inventory, ItemData, ItemRules, SlotData, DEFAULT_INVENTORY_SIZE};
use game::player::PlayerGameMode;
use game::types::{Position, Rotation};
use game::world::chunk_position::ChunkPosition;
use game::world::data::block::BlockInstance;
use game::world::data::chunk::Chunk;
use game::world::raycast::voxel_raycast;
use game::world::world_position::WorldPosition;
use game::world::World;
use network::messages::{new_inventory_update_paquet, Paquet};
use physics::{body::PhysicsBody, collision::resolve_collision};
use project_core::general_types::updatable::Updatable;
use project_core::{log_client, log_err_client};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// État pur du joueur : position, caméra, contrôleurs, distances de rendu.
/// Séparé de `Player` pour permettre l'ajout d'un corps physique sans tout casser.
pub struct PlayerState {
    pub player_id: u64,
    pub pos: Updatable<Position>,
    pub cpos: Updatable<ChunkPosition>,
    pub game_mode: PlayerGameMode,
    pub inventory: Inventory,
    pub horizontal_render_distance: u16,
    pub vertical_render_distance: u16,
    pub horizontal_simulation_distance: u16,
    pub vertical_simulation_distance: u16,
    pub camera_controller: Box<dyn CameraController>,
    pub player_controller: Box<dyn PlayerController>,
    pub camera: Camera,
    pub selected_slot: usize,
}

impl PlayerState {
    pub fn new(
        player_id: u64,
        camera_controller: Box<dyn CameraController>,
        player_controller: Box<dyn PlayerController>,
        spawn_pos: Position,
    ) -> Self {
        Self {
            player_id,
            game_mode: PlayerGameMode::Survival,
            inventory: Inventory::default(DEFAULT_INVENTORY_SIZE),
            pos: Updatable::new(spawn_pos),
            cpos: Updatable::new((&spawn_pos).into()),
            horizontal_render_distance: HORIZONTAL_RENDER_DISTANCE,
            vertical_render_distance: VERTICAL_RENDER_DISTANCE,
            horizontal_simulation_distance: HORIZONTAL_SIMULATION_DISTANCE,
            vertical_simulation_distance: VERTICAL_SIMULATION_DISTANCE,
            camera_controller,
            player_controller,
            camera: Camera::new(spawn_pos, 1.0),
            selected_slot: 0,
        }
    }

    /// Met à jour la caméra (yaw/pitch).
    /// La position et `cpos` sont mis à jour dans `physics_update()` (timestep fixe).
    pub fn update(&mut self, dt: f32, world: &mut ClientWorld, inputs: &mut InputState) -> Vec<Paquet> {
        let position = *self.get_pos();
        self.camera_controller.update(dt, inputs, &mut self.camera, &position);

        let mut commands = Vec::new();

        let scroll = inputs.take_mouse_wheel_delta();
        if scroll != 0.0 {
            const SLOT_COUNT: u32 = 8;
            if scroll > 0.0 {
                self.selected_slot = ((self.selected_slot as u32 + SLOT_COUNT - 1) % SLOT_COUNT) as usize;
            } else {
                self.selected_slot = ((self.selected_slot as u32 + 1) % SLOT_COUNT) as usize;
            }
        }

        if inputs.take_key_pressed(KeyCode::KeyE) {
            log_client!("{}", self.inventory);
        }

        if inputs.take_mouse_button_pressed(MouseButton::Left) {
            self.break_block(world, &mut commands);
        }

        if inputs.take_mouse_button_pressed(MouseButton::Right) {
            if let Some(slot_item_stack) = self.inventory.get_slot(self.selected_slot) {
                let item_id = slot_item_stack.item().get_item().get_id();
                if let Some(block_id) = world.item_to_block(&item_id) {
                    let block = BlockInstance::new(block_id);
                    self.place_block(block, world, &mut commands);
                }
            }
        }

        commands
    }

    fn break_block(&mut self, world: &mut ClientWorld, commands: &mut Vec<Paquet>) {
        let hit = voxel_raycast(self.camera.eye(), &self.camera.forward(), 4.0, |position| {
            world.get_block(position).is_some_and(|block| block.is_solid())
        });
        let Some(hit) = hit else { return };
        let Some(block_id) = world.get_block(&hit.block_pos).map(|block| block.get_block_id()) else {
            return;
        };
        let item = world.block_to_item(&block_id);
        if let Some(item) = item {
            log_client!("Ajout de l'item id n°{:?} dans l'inventaire", item.id);
            if let Some(slot) = self.inventory.add_item(ItemData::new(item, None), 1, &ItemRules::default()) {
                // TODO: get item rules based on item
                commands.push(new_inventory_update_paquet(self.player_id, vec![slot]));
            }
        }

        match world.set_block(&hit.block_pos, BlockInstance::air()) {
            Ok(_) => {
                commands.push(GameProtocol::create_block_modification(hit.block_pos, BlockInstance::air()));
            }
            Err(chunk) => {
                let current_block = world.get_block(&hit.block_pos);
                log_err_client!(
                    "Couldn't break block {:?} at {:?} in chunk {:?}.",
                    current_block,
                    hit.block_pos,
                    chunk
                );
            }
        }
    }

    fn place_block(&mut self, block: BlockInstance, world: &mut ClientWorld, commands: &mut Vec<Paquet>) {
        let hit = voxel_raycast(self.camera.eye(), &self.camera.forward(), 4.0, |position| {
            world.get_block(position).is_some_and(|block| block.is_solid())
        });
        let Some(hit) = hit else { return };
        let (x, y, z) = (&hit.block_pos).into();
        let (d, u, v) = hit.normal.into();
        let (dx, uy, vz) = (x - d, y - u, z - v);

        let item = world.block_to_item(&block.get_block_id());
        if let Some(item) = item {
            log_client!("Retrait de l'item id n°{:?} de l'inventaire", item.id);
            if let Some(updated_stack) = self.inventory.remove_item(ItemData::new(item, None), 1, self.selected_slot) {
                commands.push(new_inventory_update_paquet(
                    self.player_id,
                    vec![SlotData {
                        index: self.selected_slot,
                        item: updated_stack,
                    }],
                ));
            }
        }

        let position = WorldPosition::new(dx, uy, vz);

        match world.set_block(&position, block) {
            Ok(_) => {
                commands.push(GameProtocol::create_block_modification(position, block));
            }
            Err(chunk) => {
                log_err_client!("Couldn't place block {:?} at {:?} in chunk {:?}.", block, position, chunk);
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
                self.set_player_controller(Box::new(SpectatorPlayerController::new(SPECTATOR_FLY_SPEED)));
                self.game_mode = PlayerGameMode::Spectator;
            }
        }
    }

    pub fn teleport(&mut self, position: Position) {
        log_client!("Téléportation du joueur de {:?} à {:?}", self.get_pos(), position);
        self.set_pos(position);
    }

    pub const fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.horizontal_render_distance = horizontal;
        self.vertical_render_distance = vertical;
    }

    pub fn set_player_controller(&mut self, player_controller: Box<dyn PlayerController>) {
        self.player_controller = player_controller;
    }

    pub const fn get_pos(&self) -> &Position {
        self.pos.current()
    }

    pub const fn get_cpos(&self) -> &ChunkPosition {
        self.cpos.current()
    }

    pub fn has_moved(&self) -> bool {
        self.pos.has_changed()
    }

    /// Réinitialise le flag `has_moved` (appelé après envoi réseau).
    pub fn reset_moved(&mut self) {
        self.pos.update(*self.pos.current());
    }

    pub fn set_pos(&mut self, position: Position) {
        self.pos.update(position);
        self.cpos.update((&position).into());
    }

    pub fn set_rot(&mut self, rot: Rotation) {
        self.camera.set_rotation(rot);
    }

    pub fn set_position_and_rotation(&mut self, position: Position, rotation: Rotation) {
        self.set_pos(position);
        self.set_rot(rotation);
    }

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à simuler autour du joueur.
    #[inline(always)]
    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        Chunk::get_cube_chunk_range(
            self.get_cpos(),
            self.horizontal_simulation_distance,
            self.vertical_simulation_distance,
        )
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à simuler autour du joueur.
    #[inline(always)]
    pub fn get_simulation_chunk_keys(&self) -> Vec<ChunkPosition> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_simulation_chunk_range();
        Chunk::get_cube_chunk_keys(min_cx, max_cx, min_cy, max_cy, min_cz, max_cz)
    }

    /// Retourne [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz]
    /// pour les chunks à rendre autour du joueur.
    #[inline(always)]
    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        Chunk::get_cube_chunk_range(
            self.get_cpos(),
            self.horizontal_render_distance,
            self.vertical_render_distance,
        )
    }

    /// Génère toutes les clés (cx, cy, cz) des chunks à afficher autour du joueur.
    #[inline(always)]
    pub fn get_rendered_chunk_keys(&self) -> Vec<ChunkPosition> {
        let [min_cx, max_cx, min_cy, max_cy, min_cz, max_cz] = self.get_rendered_chunk_range();
        Chunk::get_cube_chunk_keys(min_cx, max_cx, min_cy, max_cy, min_cz, max_cz)
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
    ) -> Self {
        let spawn_pos = Position::new(SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z);
        Self {
            state: PlayerState::new(player_id, camera_controller, player_controller, spawn_pos),
            physics_body: PhysicsBody::new(spawn_pos, 0.49),
        }
    }

    /// Délègue la mise à jour de la caméra à `PlayerState` (yaw/pitch).
    pub fn update(&mut self, dt: f32, world: &mut ClientWorld, inputs: &mut InputState) -> Vec<Paquet> {
        self.state.update(dt, world, inputs)
    }

    /// Met à jour la physique à timestep fixe :
    /// 1. Le contrôleur joueur modifie la vélocité du `PhysicsBody`
    /// 2. `resolve_collision` traduit la vélocité en déplacement et corrige les collisions
    pub fn physics_update(&mut self, dt: f32, inputs: &mut InputState, world: &ClientWorld, player_game_mode: PlayerGameMode) {
        self.state
            .player_controller
            .update(dt, inputs, &mut self.physics_body, &self.state.camera);

        let mut new_pos = *self.get_pos();
        match player_game_mode {
            PlayerGameMode::Spectator => {
                new_pos += *self.physics_body.velocity.current() * dt;
            }
            PlayerGameMode::Survival => {
                resolve_collision(world, &mut self.physics_body, dt, &mut new_pos);
            }
        }

        self.set_pos(new_pos);
    }

    /// Délègue à `self.state`.
    pub const fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.state.set_render_distance(horizontal, vertical);
    }

    pub const fn get_pos(&self) -> &Position {
        self.state.get_pos()
    }

    pub const fn get_cpos(&self) -> &ChunkPosition {
        self.state.get_cpos()
    }

    pub fn has_moved(&self) -> bool {
        self.state.has_moved()
    }

    pub fn set_pos(&mut self, pos: Position) {
        self.state.set_pos(pos);
    }

    pub fn teleport(&mut self, pos: Position) {
        self.state.teleport(pos);
    }

    pub fn get_simulation_chunk_range(&self) -> [i32; 6] {
        self.state.get_simulation_chunk_range()
    }

    pub fn get_rendered_chunk_keys(&self) -> Vec<ChunkPosition> {
        self.state.get_rendered_chunk_keys()
    }

    pub fn get_simulation_chunk_keys(&self) -> Vec<ChunkPosition> {
        self.state.get_simulation_chunk_keys()
    }

    pub fn get_rendered_chunk_range(&self) -> [i32; 6] {
        self.state.get_rendered_chunk_range()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::controllers::spectator::FreeCameraController;

    fn new_state(spawn_pos: Position) -> PlayerState {
        PlayerState::new(
            0,
            Box::new(FreeCameraController::new(0.00390625)),
            Box::new(WalkPlayerController),
            spawn_pos,
        )
    }

    /// Reproduit le test d'appartenance utilisé par l'élagage de rendu.
    fn is_in_render_range(range: [i32; 6], chunk: &ChunkPosition) -> bool {
        let (x, y, z) = chunk.into();
        x >= range[0] && x <= range[1] && y >= range[2] && y <= range[3] && z >= range[4] && z <= range[5]
    }

    #[test]
    fn test_render_range_contains_player_chunk_on_spawn() {
        // Régression : la boîte de rendu doit contenir le chunk du joueur dès
        // la première frame. Avant, un cache mal initialisé éliminait le monde
        // entier de l'écran jusqu'au premier changement de chunk du joueur.
        let state = new_state(Position::new(0.5, 12.0, 0.5));
        let range = state.get_rendered_chunk_range();
        let cpos = state.cpos.current();

        assert!(
            is_in_render_range(range, cpos),
            "le chunk du joueur doit appartenir à la boîte de rendu, sinon rien n'est affiché"
        );
    }

    #[test]
    fn test_render_range_follows_teleportation() {
        // Régression : après une téléportation, la boîte de rendu doit suivre la
        // nouvelle position, y compris si le joueur change de chunk.
        let mut state = new_state(Position::new(0.5, 12.0, 0.5));
        state.set_pos(Position::new(100.5, 12.0, -64.5));

        let cpos = state.cpos.current();
        assert_eq!(
            *cpos,
            ChunkPosition::new(3, 0, -3),
            "la position en chunk doit suivre la téléportation"
        );

        let range = state.get_rendered_chunk_range();
        assert!(
            is_in_render_range(range, cpos),
            "la boîte de rendu doit contenir le chunk de destination"
        );
    }

    #[test]
    fn test_render_range_stays_consistent_when_chunk_does_not_change() {
        // Se déplacer à l'intérieur du même chunk ne doit pas modifier la
        // boîte de rendu.
        let mut state = new_state(Position::new(0.5, 12.0, 0.5));
        let before = state.get_rendered_chunk_range();

        state.set_pos(Position::new(5.5, 13.0, 3.5));
        assert_eq!(state.get_rendered_chunk_range(), before);
    }
}
