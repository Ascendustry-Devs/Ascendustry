use cgmath::Vector3;

use crate::world::data::chunk::CHUNK_SIZE_F;

pub const HORIZONTAL_RENDER_DISTANCE: u16 = 3;
pub const VERTICAL_RENDER_DISTANCE: u16 = 1;
pub const HORIZONTAL_SIMULATION_DISTANCE: u16 = HORIZONTAL_RENDER_DISTANCE + 2;
pub const VERTICAL_SIMULATION_DISTANCE: u16 = VERTICAL_RENDER_DISTANCE + 2;

pub const HORIZONTAL_SIMULATION_DISTANCE_HALFED_F: f32 = (HORIZONTAL_SIMULATION_DISTANCE as f32) / 2.0;
pub const VERTICAL_SIMULATION_DISTANCE_HALFED_F: f32 = (VERTICAL_SIMULATION_DISTANCE as f32) / 2.0;

pub const SIMULATION_DISTANCE_IN_BLOCKS_HALFED_VEC3_F: Vector3<f32> = Vector3::new(
    HORIZONTAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
    VERTICAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
    HORIZONTAL_SIMULATION_DISTANCE_HALFED_F * CHUNK_SIZE_F,
);

pub const CHUNK_PRIORITY_DISTANCE: f32 = 32.0;
pub const CHUNK_PRIORITY_DISTANCE_SQR: f32 = CHUNK_PRIORITY_DISTANCE * CHUNK_PRIORITY_DISTANCE;

pub const MAX_MESHING_CHUNKS_IN_QUEUE: u32 = {
    let h_chunks = HORIZONTAL_RENDER_DISTANCE as u32;
    let v_chunks = VERTICAL_RENDER_DISTANCE as u32;
    h_chunks * h_chunks * v_chunks
};

pub const MAX_GENERATION_CHUNKS_IN_QUEUE: u32 = {
    let h_chunks = HORIZONTAL_SIMULATION_DISTANCE as u32;
    let v_chunks = VERTICAL_SIMULATION_DISTANCE as u32;
    h_chunks * h_chunks * v_chunks
};

// Player physics

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_WIDTH: f32 = 0.7;

/// Petit epsilon anti-interpénétration flottante
pub const COLLISION_EPSILON: f32 = 1e-3;

/// Gravité appliquée au joueur en survival (m/s²)
pub const GRAVITY: f32 = -30.0;

/// Vitesse de saut initiale (m/s)
pub const JUMP_SPEED: f32 = 8.0;

/// Vitesse de marche maximale (m/s)
pub const WALK_SPEED: f32 = 6.3;

/// Coefficient de décélération (m/s). Valeur basée sur 60 fps + frame independant.
pub const DECEL_COEF: f32 = 3.69319145e-7; // 0.78125^60

/// Position de spawn initiale du joueur (axe X)
pub const SPAWN_POSITION_X: f32 = 0.5;

/// Position de spawn initiale du joueur (axe Y) — hauteur de départ pour la recherche de spawn safe
pub const SPAWN_POSITION_Y: f32 = 0.0;

/// Position de spawn initiale du joueur (axe Z)
pub const SPAWN_POSITION_Z: f32 = 0.5;

/// Hauteur des yeux du joueur par rapport à ses pieds
pub const PLAYER_EYE_HEIGHT: f32 = 1.4;

/// Limite maximale de hauteur pour la recherche de point de spawn sûr
pub const MAX_SPAWN_SEARCH_HEIGHT: f32 = 200.0;

/// Intervalle en millisecondes entre deux cycles de validation (guard cycle)
pub const GUARD_CYCLE_INTERVAL_MS: u64 = 200;

/// Multiplicateur de tolérance pour la plausibilité des déplacements (anti-cheat)
pub const MOVEMENT_PLAUSIBILITY_MULTIPLIER: f32 = 3.5;

// Géométrie

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
