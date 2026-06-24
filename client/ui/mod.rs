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
use game::inventory::{Inventory, Item};
use rustc_hash::{FxBuildHasher, FxHashMap};

pub struct ClientUI {
    item_textures: FxHashMap<Item, u32>,
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
    pub fn new(texture_manager: &mut TextureManager) -> Self {
        let mut instance = Self::default();
        let items = [
            (Item::Dirt, "assets/images/dirt.png"),
            (Item::Grass, "assets/images/grass.png"),
            (Item::Stone, "assets/images/stone.png"),
        ];

        for (item, path) in items {
            if let Ok(img) = image::open(path) {
                let rgba = img.into_rgba8();
                let (w, h) = rgba.dimensions();
                match texture_manager.register_atlas(&rgba.into_raw(), w, h) {
                    Ok(id) => {
                        instance.item_textures.insert(item, id.id());
                    }
                    Err(e) => {
                        println!("error when registering texture {:?} {:?} : {:?}", item, path, e);
                    }
                }
            }
        }

        instance
    }

    pub fn update(&mut self, inventory: &Inventory, selected_slot: u32, renderer: &mut Renderer) {
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
                if let Some(&tex) = self.item_textures.get(&stack.item().get_item()) {
                    commands.push(DrawCommand::TexturedPanel {
                        transform: icon,
                        texture: tex,
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
