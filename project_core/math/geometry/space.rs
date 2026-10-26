use crate::general_types::vec3::Vec3;
use crate::typed_vec3;

typed_vec3!(Pos3);
typed_vec3!(Rot3);

impl<T> From<Pos3<T>> for Rot3<T> {
    fn from(value: Pos3<T>) -> Self {
        Self(value.0)
    }
}

impl<T> From<Rot3<T>> for Pos3<T> {
    fn from(value: Rot3<T>) -> Self {
        Self(value.0)
    }
}
