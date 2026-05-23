use crate::collision_world::CollisionWorld;
use satiscore::constants::{COLLISION_EPSILON, MAX_SPAWN_SEARCH_HEIGHT, PLAYER_HEIGHT, PLAYER_WIDTH};

pub fn is_position_free(world: &impl CollisionWorld, x: f32, y: f32, z: f32) -> bool {
    let half_width = PLAYER_WIDTH / 2.0;
    let min_x = (x - half_width).floor() as i32;
    let max_x = (x + half_width - COLLISION_EPSILON).floor() as i32;
    let min_y = y.floor() as i32;
    let max_y = (y + PLAYER_HEIGHT - COLLISION_EPSILON).floor() as i32;
    let min_z = (z - half_width).floor() as i32;
    let max_z = (z + half_width - COLLISION_EPSILON).floor() as i32;

    for bx in min_x..=max_x {
        for by in min_y..=max_y {
            for bz in min_z..=max_z {
                if world.is_block_solid(bx, by, bz) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn find_safe_spawn_point(world: &impl CollisionWorld, x: f32, start_y: f32, z: f32) -> (f32, f32, f32) {
    let mut y = start_y;
    while y < MAX_SPAWN_SEARCH_HEIGHT {
        if is_position_free(world, x, y, z) {
            return (x, y, z);
        }
        y += 1.0;
    }
    (x, start_y, z)
}
