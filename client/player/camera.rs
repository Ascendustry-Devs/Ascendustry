use std::f32::consts::PI;

use engine::render::camera::OPENGL_TO_WGPU_MATRIX;
use game::{
    constants::UP,
    types::{Position, Rotation},
};
use physics::aabb::AABB;
use project_core::{
    general_types::{
        mat4::{look_to_rh, perspective, Mat4},
        updatable::Updatable,
        vec3::Vec3,
    },
    math::geometry::plane::Plane,
};

#[derive(Clone)]
pub struct Camera {
    pub eye: Updatable<Position>,
    pub rotation: Updatable<Rotation>,
    pub fovy: Updatable<f32>,
    pub aspect: Updatable<f32>,
    znear: f32,
    zfar: f32,
    view_proj: Updatable<Mat4<f32>>,
    frustum_planes: Updatable<[Plane; 6]>,
}

impl Camera {
    pub fn new(eye: Position, aspect: f32) -> Self {
        Self {
            eye: Updatable::new(eye),
            rotation: Updatable::new(Rotation::new(0.0, 0.0)),
            fovy: Updatable::new(70.0),
            aspect: Updatable::new(aspect),
            znear: 0.125,
            zfar: 500.0,
            view_proj: Updatable::new(Mat4::zero()),
            frustum_planes: Updatable::new([Plane::zero(); 6]),
        }
    }

    pub const fn eye(&self) -> &Position {
        self.eye.current()
    }

    pub const fn yaw(&self) -> f32 {
        self.rotation.current().yaw
    }

    pub const fn pitch(&self) -> f32 {
        self.rotation.current().pitch
    }

    pub fn forward(&self) -> Vec3<f32> {
        let (yaw, pitch) = self.rotation.current().into();
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();

        Vec3::new(cy * cp, sp, sy * cp).normalize()
    }

    pub fn right(&self) -> Vec3<f32> {
        self.forward().cross(&UP).normalize()
    }

    pub fn target(&self) -> Position {
        *self.eye.current() + self.forward()
    }

    pub fn needs_new_view_proj(&self) -> bool {
        self.eye.has_changed() || self.rotation.has_changed() || self.aspect.has_changed() || self.fovy.has_changed()
    }

    pub fn set_view_proj(&mut self) {
        let eye = self.eye.current();
        let view = look_to_rh(eye, &self.forward(), &UP);
        let proj = perspective(
            (*self.fovy.current()) * PI / 180.0,
            *self.aspect.current(),
            self.znear,
            self.zfar,
        );
        self.view_proj.update(OPENGL_TO_WGPU_MATRIX * proj * view);
    }

    pub const fn view_proj(&self) -> &Updatable<Mat4<f32>> {
        &self.view_proj
    }

    pub fn set_position(&mut self, position: Position) {
        self.eye.update(position);
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation.update(rotation);
    }

    pub const fn get_rotation(&self) -> &Rotation {
        self.rotation.current()
    }

    // utiliser dans Camera update si view proj a changé
    pub fn set_frustum_planes(&mut self) {
        let m = *self.view_proj.current();

        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));

        // Extraction Gribb-Hartmann : plan = ligne 3 ± ligne i.
        let extract = |r: [f32; 4], sign: f32| Plane {
            normal: Vec3::new(r3[0] + sign * r[0], r3[1] + sign * r[1], r3[2] + sign * r[2]),
            d: r3[3] + sign * r[3],
        };

        let new = [
            extract(r0, 1.0),  // left
            extract(r0, -1.0), // right
            extract(r1, 1.0),  // bottom
            extract(r1, -1.0), // top
            // Clip-space WebGPU : z ∈ [0, w], donc near = ligne 2 seule.
            Plane {
                normal: Vec3::new(r2[0], r2[1], r2[2]),
                d: r2[3],
            }, // near
            extract(r2, -1.0), // far
        ]
        .map(Plane::normalize);
        self.frustum_planes.update(new);
    }

    #[inline(never)]
    pub const fn get_frustum_planes(&self) -> &[Plane; 6] {
        self.frustum_planes.current()
    }

    #[inline(never)]
    pub fn get_frustum_aabb(&self) -> AABB {
        let fov = *self.fovy.current();
        let aspect = *self.aspect.current();
        let znear = self.znear;
        let zfar = self.zfar;
        let eye = *self.eye.current();
        let forward = self.forward();
        let right = self.right();
        let up = right.cross(&forward);

        let tan_half_fov = (fov / 2.0_f32).to_radians().tan();
        let nh = tan_half_fov * znear;
        let nw = nh * aspect;
        let fh = tan_half_fov * zfar;
        let fw = fh * aspect;

        let nc = eye + forward * znear;
        let fc = eye + forward * zfar;

        // 8 coins dans les 4 directions × near/far
        let rn = right * nw;
        let rn2 = right * fw;
        let un = up * nh;
        let un2 = up * fh;

        let mut min = Position::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Position::new(f32::MIN, f32::MIN, f32::MIN);

        for c in [
            (nc - rn) - un,
            (nc + rn) - un,
            (nc - rn) + un,
            (nc + rn) + un,
            (fc - rn2) - un2,
            (fc + rn2) - un2,
            (fc - rn2) + un2,
            (fc + rn2) + un2,
        ] {
            min = Position::new(min.x().min(*c.x()), min.y().min(*c.y()), min.z().min(*c.z()));
            max = Position::new(max.x().max(*c.x()), max.y().max(*c.y()), max.z().max(*c.z()));
        }

        AABB { min, max }
    }
}
