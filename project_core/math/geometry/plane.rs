use cgmath::{InnerSpace, Vector3};

#[derive(PartialEq, Copy, Clone)]
pub struct Plane {
    pub normal: Vector3<f32>,
    pub d: f32,
}

impl Plane {
    pub const fn zero() -> Self {
        Self {
            normal: Vector3::new(0.0, 0.0, 0.0),
            d: 0.0,
        }
    }

    pub fn normalize(self) -> Self {
        let len = self.normal.magnitude();
        Self {
            normal: self.normal / len,
            d: self.d / len,
        }
    }

    pub fn distance(&self, p: Vector3<f32>) -> f32 {
        self.normal.dot(p) + self.d
    }
}
