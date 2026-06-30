use std::collections::HashMap;

use engine::{
    gpu::textures::manager::TextureManager,
    render::{
        render::Renderer,
        ui::{
            interpreter::{compiler::UiCompiler, translator::UiTranslator},
            widgets::{DrawCommand, WidgetTransform},
        },
    },
};
use game::inventory::item_manager::ItemManager;
use game::inventory::Inventory;
use rustc_hash::{FxBuildHasher, FxHashMap};

pub struct ClientUI {
    item_textures: FxHashMap<u32, u32>,
    screen_size: (u32, u32),
    vertices: Vec<u8>,
}

impl Default for ClientUI {
    fn default() -> Self {
        Self {
            item_textures: HashMap::with_hasher(FxBuildHasher),
            screen_size: (0, 0),
            vertices: Vec::new(),
        }
    }
}

impl ClientUI {
    pub fn new() -> Self {
        Self::default()
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

    pub fn update(&mut self, inventory: &Inventory, selected_slot: u32, renderer: &mut Renderer, item_manager: &ItemManager) {
        let w = renderer.gpu_context.config.width;
        let h = renderer.gpu_context.config.height;
        if w == 0 || h == 0 {
            return;
        }
        let selected_slot = selected_slot as usize;
        self.screen_size = (w, h);

        let mut commands = Vec::with_capacity(16);
        let slot = 60u32;
        let gap = 6u32;
        let total = 8 * slot + 7 * gap;
        let start_x = w.saturating_sub(total) / 2;
        let start_y = h.saturating_sub(slot + 8);

        for i in 0..8usize {
            let x = start_x + i as u32 * (slot + gap);
            let bg = WidgetTransform::new(x, start_y, slot, slot);
            commands.push(DrawCommand::Panel {
                transform: bg,
                color: if i == selected_slot { 0xFF0000FF } else { 0xAA333333 },
            });

            if let Some(stack) = inventory.get_slot(i) {
                let icon = WidgetTransform::new(x + 6, start_y + 6, slot - 12, slot - 12);
                let item_id = stack.item().get_item().get_id();
                let tex = self.ensure_item_texture(item_id, item_manager, &mut renderer.texture_manager);
                if let Some(tex_id) = tex {
                    commands.push(DrawCommand::TexturedPanel {
                        transform: icon,
                        texture: tex_id,
                    });
                } else {
                    commands.push(DrawCommand::Panel {
                        transform: icon,
                        color: 0xFFAAAAAA,
                    });
                }
            }
        }

        let vertices = UiTranslator::translate(commands, &renderer.texture_manager);
        let bytes = UiCompiler::compile(vertices);
        self.vertices = bytes;
        renderer.ui_renderer.update_vertices(&self.vertices);
    }
}
