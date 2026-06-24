/// A 2D rectangle.
///
/// - `(x, y)` is the top-left corner.
/// - `(w, h)` are the left-to-right, top-to-bottom dimensions of the rectangle.
pub struct Rect2 {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl Rect2 {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn x(&self) -> i32 {
        self.x
    }

    pub const fn y(&self) -> i32 {
        self.y
    }

    pub const fn w(&self) -> u32 {
        self.w
    }

    pub const fn h(&self) -> u32 {
        self.h
    }
}
