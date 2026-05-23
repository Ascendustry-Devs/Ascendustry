use cgmath::{Point3, Vector3};

pub struct AABB {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

#[allow(unused)]
impl AABB {
    pub fn new(center: Point3<f32>, half_size: f32) -> Self {
        Self::new_sized(center, Vector3::new(half_size, half_size, half_size))
    }

    pub fn new_sized(center: Point3<f32>, half_size: Vector3<f32>) -> Self {
        Self {
            min: Vector3::new(center.x - half_size.x, center.y - half_size.y, center.z - half_size.z),
            max: Vector3::new(center.x + half_size.x, center.y + half_size.y, center.z + half_size.z),
        }
    }

    pub fn translate(&mut self, v: Vector3<f32>) {
        self.max += v;
        self.min += v;
    }

    pub fn overlaps(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn corners(&self) -> [Vector3<f32>; 8] {
        let [min, max] = [&self.min, &self.max];
        [
            Vector3::new(min.x, min.y, min.z),
            Vector3::new(max.x, min.y, min.z),
            Vector3::new(min.x, max.y, min.z),
            Vector3::new(max.x, max.y, min.z),
            Vector3::new(min.x, min.y, max.z),
            Vector3::new(max.x, min.y, max.z),
            Vector3::new(min.x, max.y, max.z),
            Vector3::new(max.x, max.y, max.z),
        ]
    }
}
