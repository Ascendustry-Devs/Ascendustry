use std::{collections::HashSet, time::Instant};

use crate::engine::render::{camera::RenderCamera, mesh::manager::MeshId};

pub struct EngineFrameData {
    pub dt: f32,
    pub fps: u32,
    pub fps_timer: f32,
    pub last_frame: Instant,
    pub frame_count: u32,
}

pub struct GameFrameData {
    pub camera: RenderCamera,
    pub visible_meshes: HashSet<MeshId>,
}

impl GameFrameData {
    pub fn blank() -> Self {
        Self {
            camera: RenderCamera::new(),
            visible_meshes: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.visible_meshes.clear();
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
