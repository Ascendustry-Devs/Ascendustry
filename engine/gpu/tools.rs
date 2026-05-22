use std::sync::Arc;

use wgpu::{Device, Queue};

pub struct GpuTools {
    device: Device,
    queue: Queue,
}

impl GpuTools {
    pub fn new(device: Device, queue: Queue) -> Self {
        Self { device, queue }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn from_arc(gpu_tools: &Arc<Self>) -> Arc<GpuTools> {
        Arc::clone(gpu_tools)
    }
}
