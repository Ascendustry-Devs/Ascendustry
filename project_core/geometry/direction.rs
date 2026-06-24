#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// -X
    Left = 0,
    /// -Y
    Bottom = 1,
    /// -Z
    Back = 2,
    /// +X
    Right = 3,
    /// +Y
    Top = 4,
    /// +Z
    Front = 5,
}

impl Direction {
    #[inline(always)]
    pub const fn from_bits_unchecked(v: u8) -> Self {
        debug_assert!(v < 6);
        unsafe { std::mem::transmute(v) }
    }

    #[inline(always)]
    pub const fn to_usize(self) -> usize {
        self as usize
    }

    #[inline(always)]
    pub const fn is_positive(self) -> bool {
        self as u8 >= 3
    }

    #[inline(always)]
    pub const fn is_negative(self) -> bool {
        (self as u8) < 3
    }
}
