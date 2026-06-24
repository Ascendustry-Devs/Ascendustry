use std::sync::Arc;

use wgpu::{Device, Queue};

pub struct GpuTools {
    device: Device,
    queue: Queue,
}

impl GpuTools {
    pub const fn new(device: Device, queue: Queue) -> Self {
        Self { device, queue }
    }

    pub const fn device(&self) -> &Device {
        &self.device
    }

    pub const fn queue(&self) -> &Queue {
        &self.queue
    }

    pub const fn device_queue(&self) -> (&Device, &Queue) {
        (&self.device, &self.queue)
    }

    pub fn from_arc(gpu_tools: &Arc<Self>) -> Arc<Self> {
        Arc::clone(gpu_tools)
    }
}
