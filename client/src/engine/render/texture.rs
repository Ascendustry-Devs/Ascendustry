use std::sync::Arc;

use anyhow::*;
use image::GenericImageView;

use crate::engine::render::{render::GpuTools, textures::array::Texture2DArray};

pub struct Texture {
    #[allow(unused)]
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub dimensions: (u32, u32),
}

#[repr(usize)]
pub enum PipelineType {
    Opaque = 0,
    AlphaCutout = 1,
    Translucent = 2,
    Billboard = 3,
    UI = 4,
}

impl PipelineType {
    pub const fn to_usize(self) -> usize {
        self as usize
    }
}

impl Texture {
    pub fn from_bytes(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8], label: &str) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    fn from_image(device: &wgpu::Device, queue: &wgpu::Queue, img: &image::DynamicImage, label: Option<&str>) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // En agrandissant les textures, on veut un effet "pixelisé", comme si on zoomait sur un bloc d'herbe dans Minecraft, sinon ça devient flou et dégueu pour un projet voxel.
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            dimensions,
        })
    }
}

pub struct TextureManager {
    gpu_resources: Arc<GpuTools>,
    arrays: Vec<Texture2DArray>,
    max_texture_size: u32,
    max_array_depth: u32,
}

#[repr(C)]
pub enum TextureArrayIndex {
    BLOCKS = 0,
    ITEMS = 1,
    BILLBOARDS = 2,
}

impl TextureArrayIndex {
    pub fn to_usize(self) -> usize {
        self as usize
    }
}

pub struct TextureID {
    array: usize,
    depth: u16,
}

impl TextureID {
    pub fn new(array: usize, depth: u16) -> Self {
        Self { array, depth }
    }
}

impl TextureManager {
    pub fn new(gpu_resources: Arc<GpuTools>, max_texture_size: u32, max_array_depth: u32) -> Self {
        let size = max_texture_size.min(32) as u16;

        let mut instance = Self {
            gpu_resources,
            arrays: vec![],
            max_texture_size,
            max_array_depth,
        };

        let blocks = instance.make_new_array("Blocks".to_string(), size, size);
        let items = instance.make_new_array("Items".to_string(), size, size);
        let billboards = instance.make_new_array("Billboards".to_string(), size, size);

        assert!(blocks == TextureArrayIndex::BLOCKS.to_usize());
        assert!(items == TextureArrayIndex::ITEMS.to_usize());
        assert!(billboards == TextureArrayIndex::BILLBOARDS.to_usize());

        instance
    }

    fn make_new_array(&mut self, label: String, width: u16, height: u16) -> usize {
        let width = width as u32;
        let height = height as u32;
        if width > self.max_texture_size || height > self.max_texture_size {
            panic!(
                "TextureManager - make_new_array: texture's dimensions exceeds what hardware supports.\nw: {} > {} or h: {} > {}",
                width, self.max_texture_size, height, self.max_texture_size
            )
        }

        let id = self.arrays.len();
        let depth = self.max_array_depth;
        let array = Texture2DArray::new(label, self.gpu_resources.device(), width, height, depth);

        self.arrays.push(array);

        id
    }

    pub fn find_place(&mut self, array_index: TextureArrayIndex) -> Result<TextureID, Error> {
        let idx = array_index.to_usize();
        if let Some(array) = self.arrays.get_mut(idx) {
            let depth = array.next_id();
            if depth > self.max_array_depth as u16 {
                return Err(Error::msg(""));
            } else {
                return Ok(TextureID::new(idx, depth));
            }
        }
        Err(Error::msg("No spot found for new texture"))
    }

    pub fn register(&mut self, array: TextureArrayIndex, texture: &[u8], width: u16, height: u16) -> TextureID {
        if texture.len() != ((width as u32) * (height as u32) * 4) as usize {
            panic!(
                "Texture data length does not match expected size for given dimensions.\n{} != {}*{}*4",
                texture.len(),
                width,
                height
            );
        }

        let id = self.find_place(array).expect("No more space available on the array");

        self.write(texture, &id);

        id
    }

    pub fn write(&mut self, texture: &[u8], id: &TextureID) {
        let Some(array) = self.arrays.get_mut(id.array) else {
            return;
        };

        array.write_at(self.gpu_resources.queue(), id.depth, texture);
    }

    pub fn get_array(&self, index: TextureArrayIndex) -> &Texture2DArray {
        self.arrays.get(index.to_usize()).unwrap()
    }

    pub fn get_array_mut(&mut self, index: TextureArrayIndex) -> &mut Texture2DArray {
        self.arrays.get_mut(index.to_usize()).unwrap()
    }
}
