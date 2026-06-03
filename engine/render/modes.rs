use std::fmt::Display;

#[repr(C)]
#[allow(unused)]
#[derive(PartialEq, Eq, Hash, Clone)]
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

impl Display for RenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            RenderMode::Opaque => "RenderMode::Opaque",
            RenderMode::AlphaCutout => "RenderMode::AlphaCutout",
            RenderMode::Translucent => "RenderMode::Translucent",
            RenderMode::Billboard => "RenderMode::Billboard",
            RenderMode::UI => "RenderMode::UI",
        })
    }
}
