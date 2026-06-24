use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, IndexFormat, Queue};

pub const BUFFER_CAPACITY_MARGIN: f32 = 1.0; // allocates only necessary space, nothing more, nothing less.
pub const BUFFER_MIN_CAPACITY: u32 = 1024 * 256; // 256kb
pub const BUFFER_MAX_CAPACITY: u32 = 1024 * 1024 * 256; // 256mb

pub struct SmartBuffer {
    buffer: Buffer,
    length: u32,
    capacity: u32,
    format: Option<IndexFormat>,
}

impl SmartBuffer {
    pub fn from_data(data: &[u8], device: &Device, queue: &Queue, format: Option<IndexFormat>, usages: BufferUsages) -> Self {
        let length = data.len() as u32;
        let capacity = ((length as f32 * BUFFER_CAPACITY_MARGIN).ceil() as u32).clamp(BUFFER_MIN_CAPACITY, BUFFER_MAX_CAPACITY);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("SmartBuffer (c: {}, l: {})", capacity, length).as_str()),
            size: capacity as u64,
            usage: usages,
            mapped_at_creation: false,
        });

        if length <= BUFFER_MAX_CAPACITY {
            queue.write_buffer(&buffer, 0, data);
        } else {
            queue.write_buffer(&buffer, 0, &data[..BUFFER_MAX_CAPACITY as usize])
        }

        Self {
            buffer,
            length,
            capacity,
            format,
        }
    }

    pub fn from_capacity(capacity_bytes: u32, device: &Device, format: Option<IndexFormat>, usages: BufferUsages) -> Self {
        let length = 0;
        let capacity =
            ((capacity_bytes as f32 * BUFFER_CAPACITY_MARGIN).ceil() as u32).clamp(BUFFER_MIN_CAPACITY, BUFFER_MAX_CAPACITY);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("SmartBuffer (c: {}, l: {})", capacity, length).as_str()),
            size: capacity as u64,
            usage: usages,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            length,
            capacity,
            format,
        }
    }

    #[inline(always)]
    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    #[inline(always)]
    pub const fn length(&self) -> u32 {
        self.length
    }

    #[inline(always)]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline(always)]
    pub const fn format(&self) -> Option<IndexFormat> {
        self.format
    }

    #[inline(always)]
    pub fn usages(&self) -> BufferUsages {
        self.buffer.usage()
    }

    pub fn update(&mut self, device: &Device, queue: &Queue, data: &[u8]) {
        let length = data.len() as u32;

        if self.capacity >= length {
            self.length = length;
            queue.write_buffer(&self.buffer, 0, data);
        } else {
            self.buffer.destroy();
            *self = Self::from_data(data, device, queue, self.format, self.usages());
        }
    }

    #[inline(always)]
    pub fn destroy(&mut self) {
        self.buffer.destroy();
    }
}
