pub enum TextureData {
    OfArray { depth: u32 },
    OfAtlas { x: u32, y: u32, width: u32, height: u32 },
}

impl TextureData {
    pub fn for_array(depth: u32) -> Self {
        Self::OfArray { depth }
    }

    pub fn for_atlas(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self::OfAtlas { x, y, width, height }
    }
}
