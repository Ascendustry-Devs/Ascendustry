use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use project_core::geometry::rect::Rect2;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    gpu::{
        textures::{
            array::Texture2DArray,
            atlas::Texture2DAtlas,
            data::TextureData::{self, OfArray, OfAtlas},
            id::TextureID,
        },
        tools::GpuTools,
    },
    render::modes::RenderMode,
};

#[allow(unused)]
pub struct TextureManager {
    gpu_resources: Arc<GpuTools>,

    max_texture_size: u32,
    max_array_depth: u32,

    opaque: Texture2DArray,
    alpha_cutout: Texture2DArray,
    translucent: Texture2DArray,
    billboard: Texture2DArray,
    ui: Texture2DAtlas,

    texture_ids: FxHashMap<TextureID, TextureData>,

    next_ui_id: u32,
}

impl TextureManager {
    pub fn new(gpu_resources: Arc<GpuTools>, max_texture_size: u32, max_array_depth: u32) -> Self {
        let device = gpu_resources.device();

        let [opaque, alpha_cutout, translucent, billboard] = {
            let width = 32;
            let height = 32;
            let depth = max_array_depth;

            let labels = [
                "Opaque Texture2DArray",
                "AlphaCutout Texture2DArray",
                "Translucent Texture2DArray",
                "Billboard Texture2DArray",
            ]
            .map(String::from);

            labels.map(|label| Texture2DArray::new(label, device, width, height, depth))
        };

        let ui = {
            // let width = max_texture_size;
            // let height = max_texture_size;
            let width = 2170;
            let height = 1132;

            Texture2DAtlas::new(String::from("UI Texture2DAtlas"), device, width, height)
        };

        Self {
            gpu_resources,
            opaque,
            alpha_cutout,
            translucent,
            billboard,
            ui,
            max_texture_size,
            max_array_depth,
            texture_ids: HashMap::with_hasher(FxBuildHasher),
            next_ui_id: 0,
        }
    }

    pub fn get_array(&self, render_mode: &RenderMode) -> &Texture2DArray {
        match render_mode {
            RenderMode::Opaque => &self.opaque,
            RenderMode::AlphaCutout => &self.alpha_cutout,
            RenderMode::Translucent => &self.translucent,
            RenderMode::Billboard => &self.billboard,
            _ => panic!(),
        }
    }

    pub const fn get_ui_atlas(&self) -> &Texture2DAtlas {
        &self.ui
    }

    // TODO: à refaire dans le futur pour coller avec la nouvelle implémentation
    pub fn get_ui_uvs(&self, id: &TextureID) -> Option<(f32, f32, f32, f32)> {
        let atlas_width = self.ui.width() as f32;
        let atlas_height = self.ui.height() as f32;
        match self.texture_ids.get(id) {
            Some(&TextureData::OfAtlas { x, y, width, height }) => Some((
                x as f32 / atlas_width,
                (x + width) as f32 / atlas_width,
                y as f32 / atlas_height,
                (y + height) as f32 / atlas_height,
            )),
            _ => None,
        }
    }

    pub fn register_array(&mut self, render_mode: RenderMode, texture: &[u8]) -> Result<TextureID, Error> {
        assert!(render_mode != RenderMode::UI);

        let array = self.get_array_mut(&render_mode);
        if texture.len() != (array.width() * array.height() * 4) as usize {
            panic!(
                "Texture data length does not match expected size for given dimensions.\n{} != {}*{}*4",
                texture.len(),
                array.width(),
                array.height()
            );
        }

        let id = self.find_place_array(&render_mode)?;
        let texture_data = TextureData::for_array(id.id());

        self.texture_ids.insert(id.clone(), texture_data);
        self.write(texture, &id);

        Ok(id)
    }

    pub fn register_atlas(&mut self, data: &[u8], width: u32, height: u32) -> Result<TextureID, Error> {
        let (x, y) = self.ui.allocate(width, height).ok_or_else(|| Error::msg("Atlas plein"))?;
        let queue = self.gpu_resources.queue();
        self.ui.write_at(queue, x, y, width, height, data);
        let id = TextureID::new(RenderMode::UI, self.next_ui_id);
        self.texture_ids
            .insert(id.clone(), TextureData::OfAtlas { x, y, width, height });
        self.next_ui_id += 1;
        Ok(id)
    }

    pub fn register_atlas_multiple(&mut self, textures: &[&[u8]], infos: &[Rect2]) -> Vec<Result<TextureID, Error>> {
        let mut output = Vec::with_capacity(infos.len());

        let queue = self.gpu_resources.queue();

        for (index, texture) in infos.iter().enumerate() {
            let width = texture.w();
            let height = texture.h();
            let new = match self.ui.allocate(width, height).ok_or_else(|| Error::msg("Atlas plein")) {
                Ok((x, y)) => {
                    self.ui.write_at(queue, x, y, width, height, textures[index]);
                    let id = TextureID::new(RenderMode::UI, self.next_ui_id);
                    self.texture_ids
                        .insert(id.clone(), TextureData::OfAtlas { x, y, width, height });
                    self.next_ui_id += 1;
                    Ok(id)
                }
                Err(e) => Err(e),
            };
            output.push(new);
        }

        output
    }

    fn write(&mut self, texture: &[u8], id: &TextureID) {
        let data = self.texture_ids.get(id).unwrap();
        match *data {
            OfAtlas { x, y, width, height } => {
                self.write_to_ui_atlas(x, y, width, height, texture);
            }
            OfArray { depth: _ } => {
                self.write_to_array(texture, id);
            }
        }
    }

    fn write_to_array(&mut self, texture: &[u8], id: &TextureID) {
        let gpu = Arc::clone(&self.gpu_resources);
        let queue = gpu.queue();
        let array = self.get_array_mut(id.render_mode());
        let depth = id.id();
        array.write_at(queue, depth, texture);
        drop(gpu);
    }

    fn write_to_ui_atlas(&mut self, x: u32, y: u32, width: u32, height: u32, texture: &[u8]) {
        let gpu = self.gpu_resources.clone();
        let queue = gpu.queue();
        let ui = self.get_ui_atlas_mut();
        ui.write_at(queue, x, y, width, height, texture);
    }

    fn find_place_array(&mut self, render_mode: &RenderMode) -> Result<TextureID, Error> {
        let depth = self.get_array_mut(render_mode).next_id();

        if depth > self.max_array_depth {
            Err(Error::msg(format!("No place found for {}.", render_mode)))
        } else {
            Ok(TextureID::new((*render_mode).clone(), depth))
        }
    }

    fn get_array_mut(&mut self, render_mode: &RenderMode) -> &mut Texture2DArray {
        match render_mode {
            RenderMode::Opaque => &mut self.opaque,
            RenderMode::AlphaCutout => &mut self.alpha_cutout,
            RenderMode::Translucent => &mut self.translucent,
            RenderMode::Billboard => &mut self.billboard,
            _ => panic!(),
        }
    }

    const fn get_ui_atlas_mut(&mut self) -> &mut Texture2DAtlas {
        &mut self.ui
    }
}
