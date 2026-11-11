use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

use crate::math::geometry::space::Pos3;

#[repr(C)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Vec3<T> {
    x: T,
    y: T,
    z: T,
}

impl<T> Vec3<T> {
    pub const fn new(x: T, y: T, z: T) -> Self {
        Self { x, y, z }
    }

    pub const fn x(&self) -> &T {
        &self.x
    }

    pub const fn y(&self) -> &T {
        &self.y
    }

    pub const fn z(&self) -> &T {
        &self.z
    }

    pub const fn x_mut(&mut self) -> &mut T {
        &mut self.x
    }

    pub const fn y_mut(&mut self) -> &mut T {
        &mut self.y
    }

    pub const fn z_mut(&mut self) -> &mut T {
        &mut self.z
    }
}

impl<T: Copy> Vec3<T> {
    /// Creates a `Vec3<T>` whose coordinates (x, y, z) are all `value`.
    ///
    /// # Example:
    ///
    /// ```ignore
    /// let v = Vec3::new(3.0_f32);
    /// assert_eq!(v, Vec3::new(3.0, 3.0, 3.0));
    /// ```
    pub const fn splat(value: T) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }
}

impl Vec3<f32> {
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

impl<T> Vec3<T>
where
    T: Mul<Output = T> + Sub<Output = T> + Copy,
{
    pub fn cross(&self, rhs: &Self) -> Self {
        let x = self.y * rhs.z - self.z * rhs.y;
        let y = self.z * rhs.x - self.x * rhs.z;
        let z = self.x * rhs.y - self.y * rhs.x;

        Self { x, y, z }
    }
}

impl<T> Vec3<T>
where
    T: Add<Output = T> + Mul<Output = T> + Copy,
{
    pub fn sqr_magnitude(&self) -> T {
        self.dot(self)
    }
}

impl Vec3<f32> {
    pub fn magnitude(&self) -> f32 {
        self.sqr_magnitude().sqrt()
    }

    pub fn normalize(&self) -> Self {
        let magnitude = self.magnitude();
        Self {
            x: self.x / magnitude,
            y: self.y / magnitude,
            z: self.z / magnitude,
        }
    }
}

pub trait Dot<O, R> {
    fn dot(&self, rhs: &O) -> R;
}

impl<T> Dot<Vec3<T>, T> for Vec3<T>
where
    T: Add<Output = T> + Mul<Output = T> + Copy,
{
    fn dot(&self, rhs: &Vec3<T>) -> T {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
}

impl<T> Dot<Pos3<T>, T> for Vec3<T>
where
    T: Add<Output = T> + Mul<Output = T> + Copy,
{
    fn dot(&self, rhs: &Pos3<T>) -> T {
        self.dot(&rhs.0)
    }
}

impl<T> Neg for Vec3<T>
where
    T: Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl<T> Add for Vec3<T>
where
    T: Add<Output = T> + Copy,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl<T> AddAssign for Vec3<T>
where
    T: AddAssign + Copy,
{
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl<T> AddAssign<Vec3<T>> for Pos3<T>
where
    T: AddAssign + Copy,
{
    fn add_assign(&mut self, rhs: Vec3<T>) {
        self.0.x += rhs.x;
        self.0.y += rhs.y;
        self.0.z += rhs.z;
    }
}

impl<T> SubAssign for Vec3<T>
where
    T: SubAssign + Copy,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl<T> Add<Pos3<T>> for Vec3<T>
where
    T: Add<Output = T> + Copy,
{
    type Output = Self;

    fn add(self, rhs: Pos3<T>) -> Self::Output {
        self.add(rhs.0)
    }
}

impl<T> Add<Vec3<T>> for Pos3<T>
where
    T: Add<Output = T> + Copy,
{
    type Output = Self;

    fn add(self, rhs: Vec3<T>) -> Self::Output {
        Self(rhs.add(self.0))
    }
}

impl<T> Sub<Pos3<T>> for Pos3<T>
where
    T: Sub<Output = T> + Copy,
{
    type Output = Vec3<T>;

    fn sub(self, rhs: Pos3<T>) -> Self::Output {
        self.0 - rhs.0
    }
}

impl<T> Sub<Vec3<T>> for Pos3<T>
where
    T: Sub<Output = T> + Copy,
{
    type Output = Self;

    fn sub(self, rhs: Vec3<T>) -> Self::Output {
        let inner = self.0 - rhs;
        Self(inner)
    }
}

impl<T> Sub<Vec3<T>> for Vec3<T>
where
    T: Sub<Output = T> + Copy,
{
    type Output = Self;

    fn sub(self, rhs: Vec3<T>) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl<T> Mul<T> for Vec3<T>
where
    T: Mul<Output = T> + Copy,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl<T> Div<T> for Vec3<T>
where
    T: Div<Output = T> + Copy,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

#[macro_export]
macro_rules! typed_vec3 {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq)]
        pub struct $name<T>(pub Vec3<T>);

        impl<T> $name<T> {
            pub const fn new(x: T, y: T, z: T) -> Self {
                let inner = Vec3::new(x, y, z);
                Self { 0: inner }
            }

            pub const fn x(&self) -> &T {
                self.0.x()
            }

            pub const fn y(&self) -> &T {
                self.0.y()
            }

            pub const fn z(&self) -> &T {
                self.0.z()
            }

            pub const fn x_mut(&mut self) -> &mut T {
                self.0.x_mut()
            }

            pub const fn y_mut(&mut self) -> &mut T {
                self.0.y_mut()
            }

            pub const fn z_mut(&mut self) -> &mut T {
                self.0.z_mut()
            }

            pub const fn as_vec(&self) -> &Vec3<T> {
                &self.0
            }
        }

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
