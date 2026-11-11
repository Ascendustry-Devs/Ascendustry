use std::fmt::{Display, Formatter, Result};

use crate::render::modes::RenderMode;

#[derive(Default, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TextureId {
    render_mode: RenderMode,
    id: u32,
}

impl TextureId {
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

impl Display for TextureId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "TextureId({:?}, {})", self.render_mode, self.id)
    }
}
