use cgmath::Point3;

use crate::game::{
    physics::{aabb::AABB, body::PhysicsBody},
    world::world::World,
};
use shared::world::data::chunk::CHUNK_SIZE;

// Petit epsilon pour éviter les micro-interpénétrations flottantes
const EPSILON: f32 = 1e-3;

// Hitbox légèrement plus petite que 1.0 pour pouvoir passer dans des trous 1×1
const PLAYER_HALF_SIZE: f32 = 0.4;

/// Construit l'AABB du joueur à partir de sa position aux pieds.
/// Le centre de la hitbox est à (x, y + half_size, z) pour que le pied soit en y.
fn aabb_at_feet(feet: &Point3<f32>) -> AABB {
    let center = Point3::new(feet.x, feet.y + PLAYER_HALF_SIZE, feet.z);
    AABB::new(center, PLAYER_HALF_SIZE)
}

/// Trouve tous les blocs solides qu'un AABB chevauche dans le monde.
/// Parcourt la bounding box 3D de floor(min) à floor(max - ε) exclusif.
/// Les chunks non encore chargés sont traités comme solides pour éviter
/// de marcher dans le vide et d'être éjecté violemment au chargement.
/// Exception : si le monde n'a aucun chunk chargé (première frame), on
/// retourne une liste vide pour éviter une éjection dans le vide.
pub fn get_colliding_blocks(world: &World, aabb: &AABB) -> Vec<(i32, i32, i32)> {
    if world.is_empty() {
        return Vec::new();
    }

    let min_x = aabb.min.x.floor() as i32;
    let max_x = (aabb.max.x - EPSILON).floor() as i32;
    let min_y = aabb.min.y.floor() as i32;
    let max_y = (aabb.max.y - EPSILON).floor() as i32;
    let min_z = aabb.min.z.floor() as i32;
    let max_z = (aabb.max.z - EPSILON).floor() as i32;

    // Aucun bloc possible si l'AABB est dégénéré ou contenu dans un seul bloc
    if min_x > max_x || min_y > max_y || min_z > max_z {
        return Vec::new();
    }

    let mut blocks = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let cx = x.div_euclid(CHUNK_SIZE);
                let cy = y.div_euclid(CHUNK_SIZE);
                let cz = z.div_euclid(CHUNK_SIZE);
                if world.get_chunk_data(cx, cy, cz).is_none() || world.get_block_from_xyz(x, y, z).is_solid() {
                    blocks.push((x, y, z));
                }
            }
        }
    }
    blocks
}

/// Résout les collisions joueur/monde avec séparation des axes.
/// Algorithme classique des voxel games : on traite X, Y, Z indépendamment
/// pour que le joueur glisse naturellement le long des murs.
///
/// 1. Applique la gravité à la vélocité Y
/// 2. Pour chaque axe : ajoute la vélocité → détecte collisions → corrige la position → annule la vélocité
///
/// Important : on filtre les blocs par direction de mouvement pour éviter que
/// `min`/`max` ne choisisse un bloc du mauvais côté dans les espaces exigus,
/// ce qui téléporterait le joueur à travers les parois.
pub fn resolve_collision(world: &World, body: &mut PhysicsBody, dt: f32, position: &mut Point3<f32>) {
    // Gravité
    body.velocity.y += body.gravity * dt;

    // Axe Y — traité en premier pour que le saut permette de monter les marches
    position.y += body.velocity.y * dt;
    if body.velocity.y > 0.0 {
        let aabb = aabb_at_feet(position);
        let nearest = get_colliding_blocks(world, &aabb)
            .iter()
            .filter(|&(_, by, _)| (*by as f32) > position.y - 2.0 * PLAYER_HALF_SIZE)
            .map(|&(_, by, _)| by)
            .min();
        if let Some(by) = nearest {
            position.y = by as f32 - 2.0 * PLAYER_HALF_SIZE - EPSILON;
            body.velocity.y = 0.0;
        }
    } else if body.velocity.y < 0.0 {
        let aabb = aabb_at_feet(position);
        let nearest = get_colliding_blocks(world, &aabb)
            .iter()
            .filter(|&(_, by, _)| (*by as f32) < position.y)
            .map(|&(_, by, _)| by)
            .max();
        if let Some(by) = nearest {
            position.y = by as f32 + 1.0 + EPSILON;
            body.on_ground = true;
            body.velocity.y = 0.0;
        }
    }

    // Axe X — on ne collisionne que les blocs dans la direction du mouvement
    position.x += body.velocity.x * dt;
    {
        let aabb = aabb_at_feet(position);
        let blocks = get_colliding_blocks(world, &aabb);
        if body.velocity.x > 0.0 {
            if let Some(bx) = blocks
                .iter()
                .filter(|&&(bx, _, _)| (bx as f32) >= position.x)
                .map(|&(bx, _, _)| bx)
                .max()
            {
                position.x = bx as f32 - PLAYER_HALF_SIZE - EPSILON;
                body.velocity.x = 0.0;
            }
        } else if body.velocity.x < 0.0 {
            if let Some(bx) = blocks
                .iter()
                .filter(|&&(bx, _, _)| (bx as f32) + 1.0 <= position.x)
                .map(|&(bx, _, _)| bx)
                .min()
            {
                position.x = bx as f32 + 1.0 + PLAYER_HALF_SIZE + EPSILON;
                body.velocity.x = 0.0;
            }
        }
    }

    // Axe Z — pareil : seuls les blocs dans la direction du mouvement
    position.z += body.velocity.z * dt;
    {
        let aabb = aabb_at_feet(position);
        let blocks = get_colliding_blocks(world, &aabb);
        if body.velocity.z > 0.0 {
            if let Some(bz) = blocks
                .iter()
                .filter(|&&(_, _, bz)| (bz as f32) >= position.z)
                .map(|&(_, _, bz)| bz)
                .max()
            {
                position.z = bz as f32 - PLAYER_HALF_SIZE - EPSILON;
                body.velocity.z = 0.0;
            }
        } else if body.velocity.z < 0.0 {
            if let Some(bz) = blocks
                .iter()
                .filter(|&&(_, _, bz)| (bz as f32) + 1.0 <= position.z)
                .map(|&(_, _, bz)| bz)
                .min()
            {
                position.z = bz as f32 + 1.0 + PLAYER_HALF_SIZE + EPSILON;
                body.velocity.z = 0.0;
            }
        }
    }
}
