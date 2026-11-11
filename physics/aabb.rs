use game::types::Position;
use project_core::general_types::vec3::Vec3;

// miss_example
/// Represents an **Axis-Aligned Bounding Box** *(AABB)* in 3D space by its two corners' `Position` `min` and `max`.
pub struct AABB {
    pub min: Position,
    pub max: Position,
}

impl AABB {
    // miss_example
    /// Creates an `AABB`.
    ///
    /// # Security
    ///
    /// `min` <= `max`
    pub const fn new(min: Position, max: Position) -> Self {
        Self { min, max }
    }

    /// Creates an `AABB` based on a cube's `center` and `half_size`.
    ///
    /// # Example:
    ///
    /// ```
    /// let v = Vec3::new(3.0_f32);
    /// assert_eq!(v, Vec3::new(3.0, 3.0, 3.0));
    /// ```
    pub fn from_cube(center: Position, half_size: f32) -> Self {
        Self::from_size(center, Vec3::splat(half_size))
    }

    // fn_miss_example
    /// Creates an `AABB` based on a `corner` and a `direction`, no matter what they are.
    pub fn from_corner_and_dir(corner: Position, direction: Vec3<f32>) -> Self {
        let half_direction = direction / 2.0;
        Self::from_size(corner + half_direction, half_direction)
    }

    // fn_miss_example
    /// Creates an `AABB` based on a `center` point and a `half_size`, no matter what they are.
    pub fn from_size(center: Position, half_size: Vec3<f32>) -> Self {
        let min = center - half_size;
        let max = center + half_size;
        Self::new(min, max)
    }

    // fn_miss_example
    /// Translates current `AABB` to `self + vector`.
    pub fn translate(&mut self, vector: Vec3<f32>) {
        self.max += vector;
        self.min += vector;
    }

    // fn_miss_example
    /// Checks whether or not current `AABB` intersects with another `AABB`.
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x() <= other.max.x()
            && self.max.x() >= other.min.x()
            && self.min.y() <= other.max.y()
            && self.max.y() >= other.min.y()
            && self.min.z() <= other.max.z()
            && self.max.z() >= other.min.z()
    }

    // fn_miss_example
    /// Creates and returns the `8` corners' `Position` of current `AABB`.
    pub const fn corners(&self) -> [Position; 8] {
        let [min, max] = [&self.min, &self.max];
        [
            Position::new(*min.x(), *min.y(), *min.z()),
            Position::new(*max.x(), *min.y(), *min.z()),
            Position::new(*min.x(), *max.y(), *min.z()),
            Position::new(*max.x(), *max.y(), *min.z()),
            Position::new(*min.x(), *min.y(), *max.z()),
            Position::new(*max.x(), *min.y(), *max.z()),
            Position::new(*min.x(), *max.y(), *max.z()),
            Position::new(*max.x(), *max.y(), *max.z()),
        ]
    }

    // fn_miss_example
    /// Creates and returns the center's `Position` of current `AABB`.
    pub fn center(&self) -> Position {
        let halfed_x = self.x_length() / 2.0;
        let halfed_y = self.y_length() / 2.0;
        let halfed_z = self.z_length() / 2.0;
        Position::new(halfed_x + self.min.x(), halfed_y + self.min.y(), halfed_z + self.min.z())
    }

    // fn_miss_example
    /// Creates and returns the x-axis `length` (or size) of current `AABB`.
    pub fn x_length(&self) -> f32 {
        (self.max.x() - self.min.x()).abs()
    }

    // fn_miss_example
    /// Creates and returns the y-axis `length` (or size) of current `AABB`.
    pub fn y_length(&self) -> f32 {
        (self.max.y() - self.min.y()).abs()
    }

    // fn_miss_example
    /// Creates and returns the z-axis `length` (or size) of current `AABB`.
    pub fn z_length(&self) -> f32 {
        (self.max.z() - self.min.z()).abs()
    }
}
