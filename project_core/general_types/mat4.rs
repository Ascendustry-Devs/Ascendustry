use std::{
    f32::consts::PI,
    ops::{AddAssign, Index, IndexMut, Mul},
};

use crate::{
    general_types::vec3::{Dot, Vec3},
    math::geometry::space::Pos3,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4<T> {
    inner: [T; 4 * 4],
}

impl<T: Copy> Mat4<T> {
    pub const fn new(values: [T; 4 * 4]) -> Self {
        Self { inner: values }
    }

    pub fn x(&self) -> &[T; 4] {
        (&self.inner[0..4]).try_into().unwrap()
    }

    pub fn y(&self) -> &[T; 4] {
        (&self.inner[4..8]).try_into().unwrap()
    }

    pub fn z(&self) -> &[T; 4] {
        (&self.inner[8..12]).try_into().unwrap()
    }

    pub fn w(&self) -> &[T; 4] {
        (&self.inner[12..16]).try_into().unwrap()
    }

    pub fn row(&self, row: usize) -> [T; 4] {
        [
            self.inner[row],
            self.inner[4 + row],
            self.inner[8 + row],
            self.inner[12 + row],
        ]
    }
}

impl Mat4<f32> {
    pub const fn zero() -> Self {
        Self { inner: [0.0; 16] }
    }
}

impl<T> IndexMut<usize> for Mat4<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

impl<T> Index<usize> for Mat4<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl<T> Mul for Mat4<T>
where
    T: Default + Copy + AddAssign + Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut result: Mat4<T> = Mat4::new([T::default(); 16]);

        for col in 0..4 {
            for row in 0..4 {
                for k in 0..4 {
                    result.inner[col * 4 + row] += self.inner[k * 4 + row] * rhs.inner[col * 4 + k];
                }
            }
        }

        result
    }
}

impl<T> From<[[T; 4]; 4]> for Mat4<T>
where
    T: Copy,
{
    fn from(value: [[T; 4]; 4]) -> Self {
        let inner = [
            value[0][0],
            value[0][1],
            value[0][2],
            value[0][3],
            value[1][0],
            value[1][1],
            value[1][2],
            value[1][3],
            value[2][0],
            value[2][1],
            value[2][2],
            value[2][3],
            value[3][0],
            value[3][1],
            value[3][2],
            value[3][3],
        ];
        Self { inner }
    }
}

impl<T> From<Mat4<T>> for [[T; 4]; 4]
where
    T: Copy,
{
    fn from(value: Mat4<T>) -> Self {
        [*value.x(), *value.y(), *value.z(), *value.w()]
    }
}

pub fn look_to_rh(eye: &Pos3<f32>, dir: &Vec3<f32>, up: &Vec3<f32>) -> Mat4<f32> {
    let f = dir.normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(&f);

    Mat4::new([
        *s.x(),
        *u.x(),
        -*f.x(),
        0.0,
        *s.y(),
        *u.y(),
        -*f.y(),
        0.0,
        *s.z(),
        *u.z(),
        -*f.z(),
        0.0,
        -s.dot(eye),
        -u.dot(eye),
        f.dot(eye),
        1.0,
    ])
}

pub fn perspective(fovy_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4<f32> {
    assert!(
        fovy_rad > 0.0,
        "The vertical field of view cannot be below zero, found: {:?}",
        fovy_rad
    );
    assert!(
        fovy_rad > 0.0 && fovy_rad < PI,
        "The vertical field of view cannot be greater than a half turn, found: {:?}",
        fovy_rad
    );

    assert!(
        aspect > 0.0,
        "The absolute aspect ratio cannot be zero, found: {:?}",
        aspect.abs()
    );
    assert!(near > 0.0, "The near plane distance cannot be below zero, found: {:?}", near);
    assert!(far > 0.0, "The far plane distance cannot be below zero, found: {:?}", far);
    assert!(
        far > near,
        "The far plane and near plane are too close, found: far: {:?}, near: {:?}",
        far,
        near
    );

    let two = 2.0;
    let halfed_fovy = fovy_rad / two;
    let f = 1.0 / halfed_fovy.tan();

    let c0r0 = f / aspect;
    let c0r1 = 0.0;
    let c0r2 = 0.0;
    let c0r3 = 0.0;

    let c1r0 = 0.0;
    let c1r1 = f;
    let c1r2 = 0.0;
    let c1r3 = 0.0;

    let c2r0 = 0.0;
    let c2r1 = 0.0;
    let c2r2 = (far + near) / (near - far);
    let c2r3 = -1.0;

    let c3r0 = 0.0;
    let c3r1 = 0.0;
    let c3r2 = (two * far * near) / (near - far);
    let c3r3 = 0.0;

    Mat4::new([
        c0r0, c0r1, c0r2, c0r3, c1r0, c1r1, c1r2, c1r3, c2r0, c2r1, c2r2, c2r3, c3r0, c3r1, c3r2, c3r3,
    ])
}

pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4<f32> {
    let two = 2.0;

    let c0r0 = two / (right - left);
    let c0r1 = 0.0;
    let c0r2 = 0.0;
    let c0r3 = 0.0;

    let c1r0 = 0.0;
    let c1r1 = two / (top - bottom);
    let c1r2 = 0.0;
    let c1r3 = 0.0;

    let c2r0 = 0.0;
    let c2r1 = 0.0;
    let c2r2 = -two / (far - near);
    let c2r3 = 0.0;

    let c3r0 = -(right + left) / (right - left);
    let c3r1 = -(top + bottom) / (top - bottom);
    let c3r2 = -(far + near) / (far - near);
    let c3r3 = 1.0;

    Mat4::new([
        c0r0, c0r1, c0r2, c0r3, c1r0, c1r1, c1r2, c1r3, c2r0, c2r1, c2r2, c2r3, c3r0, c3r1, c3r2, c3r3,
    ])
}
