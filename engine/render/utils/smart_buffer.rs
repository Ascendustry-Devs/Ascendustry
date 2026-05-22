use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, IndexFormat, Queue};

pub const BUFFER_CAPACITY_MARGIN: f32 = 2.0;
pub const BUFFER_MIN_CAPACITY: u32 = 1024 * 1024 * 32;

pub struct SmartBuffer {
    buffer: Buffer,
    length: u32,
    capacity: u32,
    format: Option<IndexFormat>,
    usage: BufferUsages,
}

impl SmartBuffer {
    pub fn from_data(data: &[u8], device: &Device, queue: &Queue, format: Option<IndexFormat>, usages: BufferUsages) -> Self {
        let length = data.len() as u32;
        let capacity = BUFFER_MIN_CAPACITY.max((length as f32 * BUFFER_CAPACITY_MARGIN).ceil() as u32);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("SmartBuffer (c: {}, l: {})", capacity, length).as_str()),
            size: capacity as u64,
            usage: usages,
            mapped_at_creation: false,
        });

        queue.write_buffer(&buffer, 0, data);

        SmartBuffer {
            buffer,
            length,
            capacity,
            format: format,
            usage: usages,
        }
    }

    pub fn from_capacity(capacity_bytes: u32, device: &Device, format: Option<IndexFormat>, usages: BufferUsages) -> Self {
        let length = 0;
        let capacity = BUFFER_MIN_CAPACITY.max((capacity_bytes as f32 * BUFFER_CAPACITY_MARGIN).ceil() as u32);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("SmartBuffer (c: {}, l: {})", capacity, length).as_str()),
            size: capacity as u64,
            usage: usages,
            mapped_at_creation: false,
        });

        SmartBuffer {
            buffer,
            length,
            capacity,
            format: format,
            usage: usages,
        }
    }

    #[inline(always)]
    pub fn buffer(&self) -> &Buffer {
        return &self.buffer;
    }

    #[inline(always)]
    pub fn length(&self) -> u32 {
        return self.length;
    }

    #[inline(always)]
    pub fn capacity(&self) -> u32 {
        return self.capacity;
    }

    #[inline(always)]
    pub fn format(&self) -> Option<IndexFormat> {
        return self.format;
    }

    #[inline(always)]
    pub fn usages(&self) -> BufferUsages {
        return self.usage;
    }

    pub fn update(&mut self, device: &Device, queue: &Queue, data: &[u8]) {
        let length = data.len() as u32;

        if self.capacity >= length {
            self.length = length;
            queue.write_buffer(&self.buffer, 0, data);
        } else {
            self.buffer.destroy();
            *self = SmartBuffer::from_data(data, device, queue, self.format, self.usage);
        }
    }

    #[inline(always)]
    pub fn destroy(&mut self) {
        self.buffer.destroy();
    }
}
