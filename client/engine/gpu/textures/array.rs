use wgpu::{
    AddressMode, Device, Extent3d, FilterMode, MipmapFilterMode, Origin3d, Queue, Sampler, SamplerDescriptor,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

pub struct Texture2DArray {
    texture: Texture,
    view: TextureView,
    sampler: Sampler,
    width: u32,
    height: u32,
    depth: u32,
    next_depth: u32,
}

impl Texture2DArray {
    pub fn new(label: String, device: &Device, width: u32, height: u32, depth: u32) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label.as_str()),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            width,
            height,
            depth,
            next_depth: 0,
        }
    }

    pub const fn next_id(&mut self) -> u32 {
        let depth = self.next_depth;
        self.next_depth += 1;
        depth
    }

    pub fn write_at(&mut self, queue: &Queue, depth: u32, data: &[u8]) {
        assert_eq!(data.len(), (self.width * self.height * 4) as usize);
        assert!(depth <= self.depth);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: depth },
                aspect: TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.width),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub const fn view(&self) -> &TextureView {
        &self.view
    }

    pub const fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn depth(&self) -> u32 {
        self.depth
    }

    pub fn dispose(&mut self) {
        self.texture.destroy();
    }
}
