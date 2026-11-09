use project_core::general_types::{normal::BinaryNormal3, vec3::Vec3};

use crate::{types::Position, world::world_position::WorldPosition};

pub struct VoxelCubicRaycastHit {
    pub block_pos: WorldPosition,
    pub normal: BinaryNormal3,
}

pub fn voxel_raycast(
    origin: &Position,
    direction: &Vec3<f32>,
    max_distance: f32,
    mut stop_condition: impl FnMut(&WorldPosition) -> bool,
) -> Option<VoxelCubicRaycastHit> {
    let mut x = origin.x().floor() as i32;
    let mut y = origin.y().floor() as i32;
    let mut z = origin.z().floor() as i32;

    let step_x = if *direction.x() > 0.0 {
        1
    } else if *direction.x() < 0.0 {
        -1
    } else {
        0
    };
    let step_y = if *direction.y() > 0.0 {
        1
    } else if *direction.y() < 0.0 {
        -1
    } else {
        0
    };
    let step_z = if *direction.z() > 0.0 {
        1
    } else if *direction.z() < 0.0 {
        -1
    } else {
        0
    };

    let t_delta_x = if step_x != 0 {
        1.0 / direction.x().abs()
    } else {
        f32::INFINITY
    };
    let t_delta_y = if step_y != 0 {
        1.0 / direction.y().abs()
    } else {
        f32::INFINITY
    };
    let t_delta_z = if step_z != 0 {
        1.0 / direction.z().abs()
    } else {
        f32::INFINITY
    };

    let dist_to_next_face_x = if step_x > 0 {
        x as f32 + 1.0 - origin.x()
    } else {
        origin.x() - x as f32
    };
    let dist_to_next_face_y = if step_y > 0 {
        y as f32 + 1.0 - origin.y()
    } else {
        origin.y() - y as f32
    };
    let dist_to_next_face_z = if step_z > 0 {
        z as f32 + 1.0 - origin.z()
    } else {
        origin.z() - z as f32
    };

    let mut t_max_x = if step_x != 0 {
        dist_to_next_face_x * t_delta_x
    } else {
        f32::INFINITY
    };
    let mut t_max_y = if step_y != 0 {
        dist_to_next_face_y * t_delta_y
    } else {
        f32::INFINITY
    };
    let mut t_max_z = if step_z != 0 {
        dist_to_next_face_z * t_delta_z
    } else {
        f32::INFINITY
    };

    let mut t = 0.0;
    let mut prev_x = x;
    let mut prev_y = y;
    let mut prev_z = z;

    loop {
        if t > max_distance {
            return None;
        }
        let position = WorldPosition::new(x, y, z);
        if stop_condition(&position) {
            return Some(VoxelCubicRaycastHit {
                block_pos: position,
                normal: (x - prev_x, y - prev_y, z - prev_z).into(),
            });
        }

        prev_x = x;
        prev_y = y;
        prev_z = z;

        if step_x != 0 && t_max_x <= t_max_y && t_max_x <= t_max_z {
            t = t_max_x;
            x += step_x;
            t_max_x += t_delta_x;
        } else if step_y != 0 && t_max_y <= t_max_z {
            t = t_max_y;
            y += step_y;
            t_max_y += t_delta_y;
        } else if step_z != 0 {
            t = t_max_z;
            z += step_z;
            t_max_z += t_delta_z;
        } else {
            return None;
        }
    }
}
