use anyhow::*;
use image::GenericImageView;
use wgpu::{Device, Queue};

use crate::engine::render::textures::array::Texture2DArray;

pub struct Texture {
    #[allow(unused)]
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub dimensions: (u32, u32),
}

pub enum PipelineType {
    Opaque = 0,
    AlphaCutout = 1,
    Translucent = 2,
    Billboard = 3,
    UI = 4,
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
    pub arrays: Vec<Texture2DArray>,
    max_texture_size: u32,
    max_array_depth: u32,
}

pub struct TextureID {
    array: usize,
    depth: u16,
}

impl TextureID {
    pub fn new(array: usize, depth: u16) -> Self {
        Self {
            array,
            depth
        }
    }
}

impl TextureManager {
    pub fn new(
        max_texture_size: u32,
        max_array_depth: u32,
    ) -> Self {
        Self {
            arrays: vec![],
            max_texture_size,
            max_array_depth,
        }
    }

    fn make_new_array(&mut self, label: String, device: &Device, width: u16, height: u16) -> usize {
        if (width as u32) > self.max_texture_size || (height as u32) > self.max_texture_size {
            panic!("Texture's dimensions to make exceeds what hardware supports.\nw: {} > {} or h: {} > {}", width, self.max_texture_size, height, self.max_texture_size)
        }
        let id = self.arrays.len();
        self.arrays.push(
            Texture2DArray::new(
                label,
                device,
                width as u32,
                height as u32,
                self.max_array_depth
            )
        );
        id
    }

    pub fn find_place(&mut self, width: u16, height: u16) -> Result<TextureID, Error> {
        let mut i = 0;
        for array in self.arrays.iter_mut() {
            if array.width() == width && array.height() == height {
                return Ok(TextureID::new(i, array.next_id()));
            }
            i += 1;
        }
        Err(Error::msg("No spot found for new texture"))
    }

    pub fn register(&mut self, device: &Device, queue: &Queue, texture: &[u8], width: u16, height: u16) -> TextureID {
        if texture.len() != ((width as u32) * (height as u32) * 4) as usize {
            panic!("Texture data length does not match expected size for given dimensions.\n{} != {}*{}*4", texture.len(), width, height);
        }

        let id = self.find_place(width, height)
            .unwrap_or_else(|_| {
                let array = self.make_new_array("Array".to_string(), device, width, height);
                let depth = self.arrays.get_mut(array).unwrap().next_id();
                TextureID::new(array, depth)
            });
        
        self.write(queue, texture, &id);

        id
    }

    pub fn write(&mut self, queue: &Queue, texture: &[u8], id: &TextureID) {
        let Some(array) = self.arrays.get_mut(id.array) else {
            return;
        };

        array.write_at(queue, id.depth, texture);
    }
}
