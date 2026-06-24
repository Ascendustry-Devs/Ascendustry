use crate::render::modes::RenderMode;

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct TextureID {
    render_mode: RenderMode,
    id: u32,
}

impl TextureID {
    pub const fn new(render_mode: RenderMode, id: u32) -> Self {
        Self { render_mode, id }
    }

    pub const fn render_mode(&self) -> &RenderMode {
        &self.render_mode
    }

    pub const fn id(&self) -> u32 {
        self.id
    }
}
