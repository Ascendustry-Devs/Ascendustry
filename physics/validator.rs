use game::constants::{MOVEMENT_PLAUSIBILITY_MULTIPLIER, WALK_SPEED};

pub fn is_movement_plausible(old_x: f32, _old_y: f32, old_z: f32, new_x: f32, _new_y: f32, new_z: f32, dt_sec: f32) -> bool {
    let dx = new_x - old_x;
    let dz = new_z - old_z;
    let horizontal_dist_sq = dx.mul_add(dx, dz * dz);
    let max_horizontal = WALK_SPEED * dt_sec * MOVEMENT_PLAUSIBILITY_MULTIPLIER;
    horizontal_dist_sq <= max_horizontal * max_horizontal
}
