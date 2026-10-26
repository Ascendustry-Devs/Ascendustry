use crate::general_types::vec3::Vec3;

pub trait SqrMagnitude<T> {
    fn sqr_magnitude(&self) -> T;
}

pub trait Magnitude<T> {
    fn magnitude(&self) -> T;
}

impl SqrMagnitude<i32> for Vec3<i32> {
    fn sqr_magnitude(&self) -> i32 {
        let x = self.x();
        let y = self.y();
        let z = self.z();
        x * x + y * y + z * z
    }
}

impl Magnitude<f32> for Vec3<i32> {
    fn magnitude(&self) -> f32 {
        (self.sqr_magnitude() as f32).sqrt()
    }
}
