use game::types::Position;
use project_core::general_types::vec3::Vec3;

pub struct AABB {
    pub min: Vec3<f32>,
    pub max: Vec3<f32>,
}

#[allow(unused)]
impl AABB {
    pub fn new(center: Position, half_size: f32) -> Self {
        Self::new_sized(center, Vec3::new(half_size, half_size, half_size))
    }

    pub fn new_from_corner_and_dir(corner: Position, dir: Vec3<f32>) -> Self {
        let half = dir / 2.0;
        Self::new_sized(corner + half, half)
    }

    pub fn new_sized(center: Position, half_size: Vec3<f32>) -> Self {
        Self {
            min: Vec3::new(
                center.x() - half_size.x(),
                center.y() - half_size.y(),
                center.z() - half_size.z(),
            ),
            max: Vec3::new(
                center.x() + half_size.x(),
                center.y() + half_size.y(),
                center.z() + half_size.z(),
            ),
        }
    }

    pub fn translate(&mut self, v: Vec3<f32>) {
        self.max += v;
        self.min += v;
    }

    #[inline(never)]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.min.x() <= other.max.x()
            && self.max.x() >= other.min.x()
            && self.min.y() <= other.max.y()
            && self.max.y() >= other.min.y()
            && self.min.z() <= other.max.z()
            && self.max.z() >= other.min.z()
    }

    pub const fn corners(&self) -> [Vec3<f32>; 8] {
        let [min, max] = [&self.min, &self.max];
        [
            Vec3::new(*min.x(), *min.y(), *min.z()),
            Vec3::new(*max.x(), *min.y(), *min.z()),
            Vec3::new(*min.x(), *max.y(), *min.z()),
            Vec3::new(*max.x(), *max.y(), *min.z()),
            Vec3::new(*min.x(), *min.y(), *max.z()),
            Vec3::new(*max.x(), *min.y(), *max.z()),
            Vec3::new(*min.x(), *max.y(), *max.z()),
            Vec3::new(*max.x(), *max.y(), *max.z()),
        ]
    }

    pub fn center(&self) -> Position {
        let halfed_x = self.x_length() / 2.0;
        let halfed_y = self.y_length() / 2.0;
        let halfed_z = self.z_length() / 2.0;
        Position::new(halfed_x + self.min.x(), halfed_y + self.min.y(), halfed_z + self.min.z())
    }

    pub fn x_length(&self) -> f32 {
        (self.max.x() - self.min.x()).abs()
    }

    pub fn y_length(&self) -> f32 {
        (self.max.y() - self.min.y()).abs()
    }

    pub fn z_length(&self) -> f32 {
        (self.max.z() - self.min.z()).abs()
    }
}
