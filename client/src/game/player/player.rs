use crate::common::utils::updatable::Updatable;
use crate::game::player::camera::{Camera, CameraController};
use crate::game::systems::inputs::InputState;
use cgmath::{num_traits::ToPrimitive, Point3, Vector3};
use shared::world::constants::{
    HORIZONTAL_RENDER_DISTANCE, HORIZONTAL_SIMULATION_DISTANCE, VERTICAL_RENDER_DISTANCE, VERTICAL_SIMULATION_DISTANCE,
};
use shared::world::data::chunk::{CHUNK_SIZE, CHUNK_SIZE_F};
use shared::*;

pub trait PlayerController {
    fn update(&self, dt: f32, inputs: &mut InputState, player_pos: &mut Point3<f32>, camera: &Camera);
}

pub struct Player {
    uuid: i32,
    pub pos: Updatable<cgmath::Point3<f32>>,
    pub cpos: Updatable<cgmath::Point3<i32>>,
    pub vel: cgmath::Vector3<f32>,
    pub horizontal_render_distance: u16,
    pub vertical_render_distance: u16,
    pub horizontal_simulation_distance: u16,
    pub vertical_simulation_distance: u16,
    pub camera_controller: Box<dyn CameraController>,
    pub player_controller: Box<dyn PlayerController>,
    pub camera: Camera,
}

impl Player {
    pub fn new(
        camera_controller: Box<dyn CameraController>,
        player_controller: Box<dyn PlayerController>,
    ) -> Player {
        return Player {
            uuid: -1,
            pos: Updatable::new(Point3::new(0.0, 0.0, 0.0)),
            cpos: Updatable::new(Point3::new(0, 0, 0)),
            vel: Vector3::new(0.0, 0.0, 0.0),
            horizontal_render_distance: HORIZONTAL_RENDER_DISTANCE,
            vertical_render_distance: VERTICAL_RENDER_DISTANCE,
            horizontal_simulation_distance: HORIZONTAL_SIMULATION_DISTANCE,
            vertical_simulation_distance: VERTICAL_SIMULATION_DISTANCE,
            camera_controller: camera_controller,
            player_controller: player_controller,
            camera: Camera::new(Point3::new(16.0, 32.0, 16.0), 1.0),
        };
    }

    pub fn update(&mut self, dt: f32, inputs: &mut InputState) {
        let pos: &Point3<f32> = &self.get_pos();
        self.set_pos(self.get_pos());
        self.camera_controller.update(dt, inputs, &mut self.camera, pos);
        self.player_controller.update(dt, inputs, self.pos.current_mut(), &self.camera);
        self.cpos.update(self.pos.current().map(|coord| coord.div_euclid(CHUNK_SIZE_F).floor() as i32));
    }

    pub fn set_render_distance(&mut self, horizontal: u16, vertical: u16) {
        self.horizontal_render_distance = horizontal;
        self.vertical_render_distance = vertical;
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

    pub fn set_pos(&mut self, pos: cgmath::Point3<f32>) {
        self.pos.update(pos);
        self.cpos
            .update(self.pos.current().map(|coord| coord.div_euclid(CHUNK_SIZE as f32).floor() as i32));
    }

    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        log_client!(
            "The player {} has been teleported from {:?} to {:?}",
            self.uuid,
            self.get_pos(),
            Point3 { x: x, y: y, z: z }
        );
        self.set_pos(Point3 { x: x, y: y, z: z });
    }

    pub fn break_block_at(_block_pos: Point3<f32>) {}

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
