use crate::collision_world::CollisionWorld;
use game::{
    constants::{COLLISION_EPSILON, MAX_SPAWN_SEARCH_HEIGHT, PLAYER_HEIGHT, PLAYER_WIDTH},
    types::Position,
    world::world_position::WorldPosition,
};

pub fn is_position_free(world: &impl CollisionWorld, position: &Position) -> bool {
    let (x, y, z) = position.into();

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
                let position = WorldPosition::new(bx, by, bz);
                if world.is_block_solid(&position) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn find_safe_spawn_point(world: &impl CollisionWorld, position: &Position) -> Position {
    let mut position = *position;
    while *position.y() < MAX_SPAWN_SEARCH_HEIGHT {
        if is_position_free(world, &position) {
            return position;
        }
        *position.y_mut() += 1.0;
    }
    position
}
