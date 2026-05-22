use std::sync::Arc;

use anyhow::*;

use crate::{gpu::tools::GpuTools, render::textures::array::Texture2DArray};

pub struct TextureManager {
    gpu_resources: Arc<GpuTools>,
    textures_arrays: Vec<Texture2DArray>,
    max_texture_size: u32,
    max_array_depth: u32,
}

#[repr(C)]
#[allow(unused)]
pub enum RenderMode {
    Opaque = 0,
    AlphaCutout = 1,
    Translucent = 2,
    Billboard = 3,
    UI = 4,
}

impl RenderMode {
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

    pub fn array(&self) -> usize {
        self.array
    }

    pub fn depth(&self) -> u16 {
        self.depth
    }
}

impl TextureManager {
    pub fn new(gpu_resources: Arc<GpuTools>, max_texture_size: u32, max_array_depth: u32) -> Self {
        let size = max_texture_size.min(32) as u16;

        let mut instance = Self {
            gpu_resources,
            textures_arrays: vec![],
            max_texture_size,
            max_array_depth,
        };

        let texture_array = instance.make_new_array("Textures".to_string(), size, size);
        assert!(texture_array == RenderMode::Opaque.to_usize());

        // let opaque = instance.make_new_array("Opaque".to_string(), size, size);
        // let alpha_cutout = instance.make_new_array("Alpha Cutout".to_string(), size, size);
        // let translucent = instance.make_new_array("Translucent".to_string(), size, size);
        // let billboard = instance.make_new_array("Billboard".to_string(), size, size);

        // assert!(opaque == TextureArrayIndex::Opaque.to_usize());
        // assert!(alpha_cutout == TextureArrayIndex::AlphaCutout.to_usize());
        // assert!(translucent == TextureArrayIndex::Translucent.to_usize());
        // assert!(billboard == TextureArrayIndex::Billboard.to_usize());

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

        let id = self.textures_arrays.len();
        let depth = self.max_array_depth;
        let array = Texture2DArray::new(label, self.gpu_resources.device(), width, height, depth);

        self.textures_arrays.push(array);

        id
    }

    fn find_place(&mut self, array_index: RenderMode) -> Result<TextureID, Error> {
        let idx = array_index.to_usize();
        if let Some(array) = self.textures_arrays.get_mut(idx) {
            let depth = array.next_id();
            if depth > self.max_array_depth as u16 {
                return Err(Error::msg(format!("No spot found for new texture.\nIndex provided: {}", idx)));
            } else {
                return Ok(TextureID::new(idx, depth));
            }
        }
        Err(Error::msg(format!("Texture array not found.\nIndex provided: {}", idx)))
    }

    pub fn register(&mut self, array: RenderMode, texture: &[u8], width: u16, height: u16) -> Result<TextureID, Error> {
        if texture.len() != ((width as u32) * (height as u32) * 4) as usize {
            panic!(
                "Texture data length does not match expected size for given dimensions.\n{} != {}*{}*4",
                texture.len(),
                width,
                height
            );
        }

        let id = self.find_place(array)?;

        self.write(texture, &id);

        Ok(id)
    }

    pub fn write(&mut self, texture: &[u8], id: &TextureID) {
        let Some(array) = self.textures_arrays.get_mut(id.array) else {
            return;
        };

        array.write_at(self.gpu_resources.queue(), id.depth, texture);
    }

    pub fn get_array(&self, index: RenderMode) -> &Texture2DArray {
        self.textures_arrays.get(index.to_usize()).unwrap()
    }

    pub fn get_array_mut(&mut self, index: RenderMode) -> &mut Texture2DArray {
        self.textures_arrays.get_mut(index.to_usize()).unwrap()
    }
}
