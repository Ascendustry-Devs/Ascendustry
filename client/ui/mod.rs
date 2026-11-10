pub mod adapters;
pub mod context;
pub mod event;
pub mod layers;
pub mod layouts;
pub mod stack;
pub mod views;

use engine::{
    gpu::textures::manager::TextureManager,
    render::ui::{
        interpreter::{compiler::UiCompiler, translator::UiTranslator},
        render::UiRenderer,
    },
};

use crate::ui::{context::UiContext, layers::DynUiLayer, stack::UiStack};

pub struct ClientUI {
    screen_size: (u32, u32),
    stack: UiStack,
    vertices: Vec<u8>,
}

impl Default for ClientUI {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientUI {
    pub const fn new() -> Self {
        Self {
            screen_size: (0, 0),
            stack: UiStack::new(),
            vertices: Vec::new(),
        }
    }

    pub fn push_layer(&mut self, layout: Box<dyn DynUiLayer>) {
        self.stack.push(layout);
    }

    pub fn pop_layer(&mut self) -> Option<Box<dyn DynUiLayer>> {
        self.stack.pop()
    }

    pub fn update(&mut self, ctx: &UiContext) {
        for layer in self.stack.iter_mut() {
            layer.update(ctx);
        }
    }

    pub fn submit(&mut self, available_space: &(u32, u32), renderer: &mut UiRenderer, textures: &TextureManager) {
        self.screen_size = *available_space;
        let (w, h) = self.screen_size;

        if (w | h) == 0 {
            return;
        }

        let mut commands = Vec::with_capacity(32);

        for layer in self.stack.iter_mut() {
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
