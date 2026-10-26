use std::{collections::HashSet, time::Instant};

use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{gpu::allocator::entry::EntryId, render::camera::RenderCamera};

pub struct EngineFrameData {
    pub dt: f32,
    pub fps: u32,
    pub fps_timer: f32,
    pub last_frame: Instant,
    pub frame_count: u32,
}

pub struct GameFrameData {
    pub camera: RenderCamera,
    pub visible_meshes: FxHashSet<EntryId>,
}

impl Default for GameFrameData {
    fn default() -> Self {
        Self::blank()
    }
}

impl GameFrameData {
    pub fn blank() -> Self {
        Self {
            camera: RenderCamera::new(),
            visible_meshes: HashSet::with_hasher(FxBuildHasher),
        }
    }
}

impl Default for EngineFrameData {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFrameData {
    pub fn new() -> Self {
        Self {
            dt: 0.0,
            fps: 0,
            fps_timer: 0.0,
            last_frame: Instant::now(),
            frame_count: 0,
        }
    }
}
