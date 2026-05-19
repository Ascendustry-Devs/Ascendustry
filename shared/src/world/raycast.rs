use cgmath::{Point3, Vector3};

pub struct RaycastHit {
    pub block_pos: (i32, i32, i32),
    pub normal: (i32, i32, i32),
}

/// Voxel raycast - DDA, 1 test par bloc traversé
pub fn voxel_raycast(
    origin: Point3<f32>,
    direction: Vector3<f32>,
    max_distance: f32,
    mut stop_condition: impl FnMut(i32, i32, i32) -> bool,
) -> Option<RaycastHit> {
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;

    // Direction de progression sur chaque axe
    let step_x = (direction.x > 0.0) as i32 * 2 - 1;
    let step_y = (direction.y > 0.0) as i32 * 2 - 1;
    let step_z = (direction.z > 0.0) as i32 * 2 - 1;

    // Distance sur le rayon pour traverser 1 bloc sur chaque axe
    // f32::MAX si l'axe est parallèle
    let t_delta_x = if direction.x != 0.0 { (1.0 / direction.x).abs() } else { f32::MAX };
    let t_delta_y = if direction.y != 0.0 { (1.0 / direction.y).abs() } else { f32::MAX };
    let t_delta_z = if direction.z != 0.0 { (1.0 / direction.z).abs() } else { f32::MAX };

    // Distance jusqu'à la première face sur chaque axe
    let mut t_max_x = if direction.x > 0.0 {
        (x as f32 + 1.0 - origin.x) * t_delta_x
    } else if direction.x < 0.0 {
        (origin.x - x as f32) * t_delta_x
    } else {
        f32::MAX
    };
    let mut t_max_y = if direction.y > 0.0 {
        (y as f32 + 1.0 - origin.y) * t_delta_y
    } else if direction.y < 0.0 {
        (origin.y - y as f32) * t_delta_y
    } else {
        f32::MAX
    };
    let mut t_max_z = if direction.z > 0.0 {
        (z as f32 + 1.0 - origin.z) * t_delta_z
    } else if direction.z < 0.0 {
        (origin.z - z as f32) * t_delta_z
    } else {
        f32::MAX
    };

    let mut prev_x = x;
    let mut prev_y = y;
    let mut prev_z = z;

    let max_steps = max_distance.ceil() as i32;

    for _ in 0..max_steps {
        if stop_condition(x, y, z) {
            return Some(RaycastHit {
                block_pos: (x, y, z),
                normal: (x - prev_x, y - prev_y, z - prev_z),
            });
        }

        prev_x = x;
        prev_y = y;
        prev_z = z;

        // Avance sur l'axe dont la face est la plus proche
        if t_max_x < t_max_y {
            if t_max_x > max_distance {
                break;
            }
            x += step_x;
            t_max_x += t_delta_x;
        } else if t_max_y < t_max_z {
            if t_max_y > max_distance {
                break;
            }
            y += step_y;
            t_max_y += t_delta_y;
        } else {
            if t_max_z > max_distance {
                break;
            }
            z += step_z;
            t_max_z += t_delta_z;
        }
    }

    None
}
