use std::fmt::{Display, Formatter, Result};

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
    pub const fn to_usize(self) -> usize {
        self as usize
    }
}

impl Display for RenderMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(match *self {
            Self::Opaque => "RenderMode::Opaque",
            Self::AlphaCutout => "RenderMode::AlphaCutout",
            Self::Translucent => "RenderMode::Translucent",
            Self::Billboard => "RenderMode::Billboard",
            Self::UI => "RenderMode::UI",
        })
    }
}
