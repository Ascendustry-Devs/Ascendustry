use cgmath::Vector3;

use crate::world::data::chunk::CHUNK_SIZE_F;

pub const HORIZONTAL_RENDER_DISTANCE: u16 = 7;
pub const VERTICAL_RENDER_DISTANCE: u16 = 5;
pub const RENDER_DISTANCE_CHUNK_COUNT: u16 = HORIZONTAL_RENDER_DISTANCE * VERTICAL_RENDER_DISTANCE;
pub const HORIZONTAL_SIMULATION_DISTANCE: u16 = HORIZONTAL_RENDER_DISTANCE + 2;
pub const VERTICAL_SIMULATION_DISTANCE: u16 = VERTICAL_RENDER_DISTANCE + 2;

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

pub const MAX_MESHING_CHUNKS_IN_QUEUE: u32 = {
    let h_chunks = HORIZONTAL_RENDER_DISTANCE as u32;
    let v_chunks = VERTICAL_RENDER_DISTANCE as u32;
    h_chunks * h_chunks * v_chunks
};

pub const MAX_GENERATION_CHUNKS_IN_QUEUE: u32 = {
    let h_chunks = HORIZONTAL_RENDER_DISTANCE as u32;
    let v_chunks = VERTICAL_RENDER_DISTANCE as u32;
    h_chunks * h_chunks * v_chunks
};

// Player physics

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_WIDTH: f32 = 0.7;

pub const COLLISION_EPSILON: f32 = 1e-3;

pub const GRAVITY: f32 = -30.0;

pub const JUMP_SPEED: f32 = 8.0;

pub const WALK_SPEED: f32 = 6.3;

pub const DECEL_COEF: f32 = 3.69319145e-7;

pub const SPAWN_POSITION_X: f32 = 0.5;
pub const SPAWN_POSITION_Y: f32 = 0.0;
pub const SPAWN_POSITION_Z: f32 = 0.5;

pub const PLAYER_EYE_HEIGHT: f32 = 1.4;

pub const MAX_SPAWN_SEARCH_HEIGHT: f32 = 200.0;

pub const GUARD_CYCLE_INTERVAL_MS: u64 = 200;

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
