use cgmath::{InnerSpace, Vector3};

pub struct Plane {
    pub normal: Vector3<f32>,
    pub d: f32,
}

impl Plane {
    pub fn normalize(self) -> Plane {
        let len = self.normal.magnitude();
        Plane {
            normal: self.normal / len,
            d: self.d / len,
        }
    }

    pub fn distance(&self, p: Vector3<f32>) -> f32 {
        self.normal.dot(p) + self.d
    }
}
