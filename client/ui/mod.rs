pub mod adapters;
pub mod context;
pub mod event;
pub mod layers;
pub mod layouts;
pub mod stack;
pub mod views;

use std::collections::HashMap;

use engine::{
    gpu::textures::manager::TextureManager,
    render::ui::{
        interpreter::{compiler::UiCompiler, translator::UiTranslator},
        render::UiRenderer,
    },
};
use game::inventory::item_manager::ItemManager;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::ui::{layers::DynUiLayer, stack::UiStack};

pub struct ClientUI {
    item_textures: FxHashMap<u32, u32>,
    screen_size: (u32, u32),
    stack: UiStack,
    vertices: Vec<u8>,
}

impl Default for ClientUI {
    fn default() -> Self {
        Self {
            item_textures: HashMap::with_hasher(FxBuildHasher),
            screen_size: (0, 0),
            stack: UiStack::default(),
            vertices: Vec::new(),
        }
    }
}

impl ClientUI {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_layer(&mut self, layout: Box<dyn DynUiLayer>) {
        self.stack.push(layout);
    }

    /// Récupère ou charge la texture d'un item pour l'UI.
    ///
    /// Le cache `item_textures` évite de re-charger une texture déjà inscrite dans l'atlas UI.
    /// Si la texture n'est pas dans le cache :
    ///   1. On récupère la définition de l'item via `item_manager`,
    ///   2. On charge l'image depuis le disque (chemin dans `texture_path`),
    ///   3. On l'enregistre dans l'atlas UI via `register_atlas`,
    ///   4. On stocke l'ID retourné dans le cache pour les appels suivants.
    /// L'atlas UI est séparé des tableaux de textures 3D
    /// d'où l'utilisation de `register_atlas` plutôt que `register_array`.
    fn ensure_item_texture(
        &mut self,
        item_id: u32,
        item_manager: &ItemManager,
        texture_manager: &mut TextureManager,
    ) -> Option<u32> {
        if let Some(&tex) = self.item_textures.get(&item_id) {
            return Some(tex);
        }

        let def = item_manager.get_by_id(item_id)?;
        let tex_path = def.texture_path.as_ref()?;

        if let Ok(img) = image::open(tex_path) {
            let rgba = img.into_rgba8();
            let (w, h) = rgba.dimensions();
            match texture_manager.register_atlas(&rgba.into_raw(), w, h) {
                Ok(id) => {
                    let tex_id = id.id();
                    self.item_textures.insert(item_id, tex_id);
                    Some(tex_id)
                }
                Err(_e) => None,
            }
        } else {
            None
        }
    }

    pub fn submit(&mut self, available_space: &(u32, u32), renderer: &mut UiRenderer, textures: &TextureManager) {
        self.screen_size = *available_space;
        let (w, h) = self.screen_size;

        if w == 0 || h == 0 {
            return;
        }

        let mut commands = Vec::with_capacity(32);

        for layer in self.stack.iter() {
            let layout = layer.layout(&self.screen_size);
            commands.append(&mut layer.draw(layout.as_ref()));
        }

        // Traduire les commandes en vertices et mettre à jour le GPU
        let vertices = UiTranslator::translate(commands, textures);
        let bytes = UiCompiler::compile(vertices);
        self.vertices = bytes;
        renderer.update_vertices(&self.vertices);
    }
}
