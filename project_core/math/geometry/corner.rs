#[repr(C)]
pub enum SquareCorner {
    TopLeft = 0,
    TopRight = 1,
    BottomLeft = 2,
    BottomRight = 3,
}

impl SquareCorner {
    #[inline(always)]
    pub const fn to_usize(self) -> usize {
        self as usize
    }
}
