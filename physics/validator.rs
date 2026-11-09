use game::{
    constants::{MOVEMENT_PLAUSIBILITY_MULTIPLIER, WALK_SPEED},
    types::Position,
};

pub fn is_movement_plausible(old_position: &Position, new_position: &Position, dt_sec: f32) -> bool {
    let (old_x, _, old_z) = old_position.into();
    let (new_x, _, new_z) = new_position.into();
    let dx = new_x - old_x;
    let dz = new_z - old_z;
    let horizontal_dist_sq = dx.mul_add(dx, dz * dz);
    let max_horizontal = WALK_SPEED * dt_sec * MOVEMENT_PLAUSIBILITY_MULTIPLIER;
    horizontal_dist_sq <= max_horizontal * max_horizontal
}
