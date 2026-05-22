use satiscore::constants::MOVEMENT_PLAUSIBILITY_MULTIPLIER;
use satiscore::{constants::WALK_SPEED, network::messages::Position};

pub fn is_movement_plausible(old: &Position, new: &Position, dt_sec: f32) -> bool {
    let dx = new.x - old.x;
    let dz = new.z - old.z;
    let horizontal_dist_sq = dx * dx + dz * dz;
    let max_horizontal = WALK_SPEED * dt_sec * MOVEMENT_PLAUSIBILITY_MULTIPLIER;
    horizontal_dist_sq <= max_horizontal * max_horizontal
}
