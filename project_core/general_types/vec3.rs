#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vec3<T> {
    x: T,
    y: T,
    z: T,
}

impl<T> Vec3<T> {
    pub fn new(x: T, y: T, z: T) -> Self {
        Self { x, y, z }
    }

    pub fn x(&self) -> &T {
        &self.x
    }

    pub fn y(&self) -> &T {
        &self.y
    }

    pub fn z(&self) -> &T {
        &self.z
    }
}

#[macro_export]
macro_rules! typed_vec3 {
    ($name:ident) => {
        #[repr(transparent)]
        pub struct $name<T>(pub Vec3<T>);

        impl<T> From<Vec3<T>> for $name<T> {
            fn from(v: Vec3<T>) -> Self {
                $name(v)
            }
        }

        impl<T> From<$name<T>> for Vec3<T> {
            fn from(a: $name<T>) -> Vec3<T> {
                a.0
            }
        }
    };
}
