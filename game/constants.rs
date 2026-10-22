use cgmath::Vector3;

use crate::world::data::chunk::CHUNK_SIZE_F;

pub const HORIZONTAL_RENDER_DISTANCE: u16 = 15;
pub const VERTICAL_RENDER_DISTANCE: u16 = 5;
pub const HORIZONTAL_SIMULATION_DISTANCE: u16 = HORIZONTAL_RENDER_DISTANCE + 2;
pub const VERTICAL_SIMULATION_DISTANCE: u16 = VERTICAL_RENDER_DISTANCE + 2;

/// Nombre de chunks couverts sur un axe horizontal par une distance donnée.
///
/// La distance est exprimée en nombre de chunks de part et d'autre du joueur :
/// `get_cube_chunk_range` utilise `distance / 2` (arrondi vers le bas), d'où
/// la formule `2 * (distance / 2) + 1`.
pub const fn horizontal_chunk_axis_count(distance: u16) -> u16 {
    (distance.div_euclid(2)) * 2 + 1
}

/// Nombre de chunks couverts sur un axe vertical par une distance donnée.
pub const fn vertical_chunk_axis_count(distance: u16) -> u16 {
    (distance.div_euclid(2)) * 2 + 1
}

/// Nombre total de chunks du cube de **rendu** (15 x 5 x 15 = 1125).
///
/// C'est le volume qui doit être **maillé** : un chunk n'est maillé que si
/// ses 6 voisins existent, donc le cube maillé est le cube de rendu.
pub const RENDER_DISTANCE_CHUNK_COUNT: u16 = {
    let h = horizontal_chunk_axis_count(HORIZONTAL_RENDER_DISTANCE);
    let v = vertical_chunk_axis_count(VERTICAL_RENDER_DISTANCE);
    h * h * v
};

/// Nombre total de chunks du cube de **simulation** (17 x 7 x 17 = 2023).
///
/// C'est le volume qui doit être **généré** : il contient le cube de rendu
/// plus une coquille de chunks, qui sert de voisinage pour le maillage.
pub const SIMULATION_DISTANCE_CHUNK_COUNT: u16 = {
    let h = horizontal_chunk_axis_count(HORIZONTAL_SIMULATION_DISTANCE);
    let v = vertical_chunk_axis_count(VERTICAL_SIMULATION_DISTANCE);
    h * h * v
};

pub const HORIZONTAL_SIMULATION_DISTANCE_HALFED_F: f32 = (HORIZONTAL_SIMULATION_DISTANCE as f32) / 2.0;
pub const VERTICAL_SIMULATION_DISTANCE_HALFED_F: f32 = (VERTICAL_SIMULATION_DISTANCE as f32) / 2.0;

pub const HALFED_SIMULATION_DISTANCE_IN_BLOCKS_VEC3_F: Vector3<f32> = Vector3::new(
    HORIZONTAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
    VERTICAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
    HORIZONTAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
);

pub const CHUNK_PRIORITY_DISTANCE: f32 = 32.0;
pub const CHUNK_PRIORITY_DISTANCE_SQR: f32 = CHUNK_PRIORITY_DISTANCE * CHUNK_PRIORITY_DISTANCE;

pub const CHUNK_VECTOR: Vector3<f32> = Vector3::new(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F);

/// Nombre maximum de chunks en attente de **maillage**.
///
/// Doit couvrir le cube de rendu : au-delà, les chunks proches ne seraient
/// jamais maillés.
pub const MAX_MESHING_CHUNKS_IN_QUEUE: u32 = RENDER_DISTANCE_CHUNK_COUNT as u32;

/// Nombre maximum de chunks en attente de **génération**.
///
/// Doit couvrir le cube de simulation : la file doit pouvoir contenir
/// l'intégralité des chunks nécessaires autour du joueur, sinon les chunks
/// les plus lointains sont systématiquement écartés.
pub const MAX_GENERATION_CHUNKS_IN_QUEUE: u32 = SIMULATION_DISTANCE_CHUNK_COUNT as u32;

// Player physics

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_WIDTH: f32 = 0.7;

pub const COLLISION_EPSILON: f32 = 1e-3;

pub const GRAVITY: f32 = -30.0;

pub const JUMP_SPEED: f32 = 8.0;

pub const WALK_SPEED: f32 = 6.3;

pub const SPECTATOR_FLY_SPEED: f32 = 30.0;

pub const DECEL_COEF: f32 = 3.693_191_4e-7;

pub const SPAWN_POSITION_X: f32 = 0.5;
pub const SPAWN_POSITION_Y: f32 = 0.0;
pub const SPAWN_POSITION_Z: f32 = 0.5;

pub const PLAYER_EYE_HEIGHT: f32 = 1.4;

pub const MAX_SPAWN_SEARCH_HEIGHT: f32 = 200.0;

pub const GUARD_CYCLE_PER_SEC: u64 = 20;
pub const GUARD_CYCLE_INTERVAL_MS: u64 = 1000 / GUARD_CYCLE_PER_SEC;

pub const MOVEMENT_PLAUSIBILITY_MULTIPLIER: f32 = 3.5;

// Geometry

pub const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

pub const DIRECT_NORMALS_3D: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
pub const INDIRECT_NORMALS_3D: [(i32, i32, i32); 20] = [
    (-1, -1, 0),
    (1, -1, 0),
    (-1, 1, 0),
    (1, 1, 0),
    (-1, 0, -1),
    (1, 0, -1),
    (0, -1, -1),
    (-1, -1, -1),
    (1, -1, -1),
    (0, 1, -1),
    (-1, 1, -1),
    (1, 1, -1),
    (-1, 0, 1),
    (1, 0, 1),
    (0, -1, 1),
    (-1, -1, 1),
    (1, -1, 1),
    (0, 1, 1),
    (-1, 1, 1),
    (1, 1, 1),
];
pub const ALL_NORMALS_3D: [(i32, i32, i32); 26] = [
    (-1, 0, 0),
    (1, 0, 0),
    (0, -1, 0),
    (-1, -1, 0),
    (1, -1, 0),
    (0, 1, 0),
    (-1, 1, 0),
    (1, 1, 0),
    (0, 0, -1),
    (-1, 0, -1),
    (1, 0, -1),
    (0, -1, -1),
    (-1, -1, -1),
    (1, -1, -1),
    (0, 1, -1),
    (-1, 1, -1),
    (1, 1, -1),
    (0, 0, 1),
    (-1, 0, 1),
    (1, 0, 1),
    (0, -1, 1),
    (-1, -1, 1),
    (1, -1, 1),
    (0, 1, 1),
    (-1, 1, 1),
    (1, 1, 1),
];
