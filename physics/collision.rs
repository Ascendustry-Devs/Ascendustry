use crate::aabb::AABB;
use crate::body::PhysicsBody;
use crate::collision_world::CollisionWorld;
use game::{
    constants::{COLLISION_EPSILON, PLAYER_HEIGHT, PLAYER_WIDTH},
    types::Position,
    world::world_position::WorldPosition,
};
use project_core::math::geometry::vec3::Vec3;

fn aabb_at_feet(feet: &Position) -> AABB {
    let half_width = PLAYER_WIDTH / 2.0;
    let half_height = PLAYER_HEIGHT / 2.0;
    let center = Position::new(*feet.x(), feet.y() + half_height, *feet.z());
    AABB::from_size(center, Vec3::new(half_width, half_height, half_width))
}

pub fn get_colliding_blocks(world: &impl CollisionWorld, aabb: &AABB) -> Vec<WorldPosition> {
    if world.is_empty() {
        return Vec::new();
    }

    let min_x = aabb.min.x().floor() as i32;
    let max_x = (aabb.max.x() - COLLISION_EPSILON).floor() as i32;
    let min_y = aabb.min.y().floor() as i32;
    let max_y = (aabb.max.y() - COLLISION_EPSILON).floor() as i32;
    let min_z = aabb.min.z().floor() as i32;
    let max_z = (aabb.max.z() - COLLISION_EPSILON).floor() as i32;

    if min_x > max_x || min_y > max_y || min_z > max_z {
        return Vec::new();
    }

    let capacity = ((max_x - min_x) * (max_y - min_y) * (max_z - min_z)) as usize;
    let mut blocks = Vec::with_capacity(capacity);
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let position = WorldPosition::new(x, y, z);
                if world.is_block_solid(&position) {
                    blocks.push(position);
                }
            }
        }
    }
    blocks
}

pub fn resolve_collision(world: &impl CollisionWorld, body: &mut PhysicsBody, dt: f32, position: &mut Position) {
    let mut velocity = *body.velocity_mut().current();
    *velocity.y_mut() += body.gravity * dt;

    *position.y_mut() += velocity.y() * dt;
    if *velocity.y() > 0.0 {
        let aabb = aabb_at_feet(position);
        let nearest = get_colliding_blocks(world, &aabb)
            .iter()
            .filter(|&c| (c.y() as f32) > position.y() - PLAYER_HEIGHT)
            .map(|&c| c.y())
            .min();
        if let Some(by) = nearest {
            *position.y_mut() = by as f32 - PLAYER_HEIGHT - COLLISION_EPSILON;
            *velocity.y_mut() = 0.0;
        }
    } else if *velocity.y() < 0.0 {
        let aabb = aabb_at_feet(position);
        let nearest = get_colliding_blocks(world, &aabb)
            .iter()
            .filter(|&c| (c.y() as f32) < *position.y())
            .map(|&c| c.y())
            .max();
        if let Some(by) = nearest {
            *position.y_mut() = by as f32 + 1.0 + COLLISION_EPSILON;
            body.on_ground = true;
            *velocity.y_mut() = 0.0;
        }
    }

    *position.x_mut() += velocity.x() * dt;
    {
        let aabb = aabb_at_feet(position);
        let blocks = get_colliding_blocks(world, &aabb);
        if *velocity.x() > 0.0 {
            if let Some(bx) = blocks
                .iter()
                .filter(|&c| (c.x() as f32) >= *position.x())
                .map(|&c| c.x())
                .max()
            {
                *position.x_mut() = bx as f32 - PLAYER_WIDTH / 2.0 - COLLISION_EPSILON;
                *velocity.x_mut() = 0.0;
            }
        } else if *velocity.x() < 0.0 {
            if let Some(bx) = blocks
                .iter()
                .filter(|&c| (c.x() as f32) + 1.0 <= *position.x())
                .map(|&c| c.x())
                .min()
            {
                *position.x_mut() = bx as f32 + 1.0 + PLAYER_WIDTH / 2.0 + COLLISION_EPSILON;
                *velocity.x_mut() = 0.0;
            }
        }
    }

    *position.z_mut() += velocity.z() * dt;
    {
        let aabb = aabb_at_feet(position);
        let blocks = get_colliding_blocks(world, &aabb);
        if *velocity.z() > 0.0 {
            if let Some(bz) = blocks
                .iter()
                .filter(|&c| (c.z() as f32) >= *position.z())
                .map(|&c| c.z())
                .max()
            {
                *position.z_mut() = bz as f32 - PLAYER_WIDTH / 2.0 - COLLISION_EPSILON;
                *velocity.z_mut() = 0.0;
            }
        } else if *velocity.z_mut() < 0.0 {
            if let Some(bz) = blocks
                .iter()
                .filter(|&c| (c.z() as f32) + 1.0 <= *position.z())
                .map(|&c| c.z())
                .min()
            {
                *position.z_mut() = bz as f32 + 1.0 + PLAYER_WIDTH / 2.0 + COLLISION_EPSILON;
                *velocity.z_mut() = 0.0;
            }
        }
    }

    body.velocity.update(velocity);
}
