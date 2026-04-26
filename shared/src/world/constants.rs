pub const HORIZONTAL_RENDER_DISTANCE: u16 = 9;
pub const VERTICAL_RENDER_DISTANCE: u16 = 5;
pub const HORIZONTAL_SIMULATION_DISTANCE: u16 = HORIZONTAL_RENDER_DISTANCE + 2;
pub const VERTICAL_SIMULATION_DISTANCE: u16 = VERTICAL_RENDER_DISTANCE + 2;

pub const MAX_CHUNKS_PER_BATCH: usize = 128;
pub const MIN_CHUNKS_PER_BATCH: usize = 4;

pub const CHUNK_PRIORITY_DISTANCE: f32 = 30.0;

pub const fn max_chunks_in_queue() -> u32 {
    let h_chunks = HORIZONTAL_SIMULATION_DISTANCE as u32;
    let v_chunks = VERTICAL_SIMULATION_DISTANCE as u32;
    h_chunks * h_chunks * v_chunks
}
