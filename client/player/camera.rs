use cgmath::{Deg, InnerSpace, Matrix4, Point3, Vector3, Vector4};
use engine::render::camera::OPENGL_TO_WGPU_MATRIX;
use game::constants::UP;
use physics::aabb::AABB;
use project_core::{geometry::plane::Plane, utils::updatable::Updatable};

#[derive(Clone)]
pub struct Camera {
    pub eye: Updatable<Point3<f32>>,
    pub yaw: Updatable<f32>,
    pub pitch: Updatable<f32>,
    pub fovy: Updatable<f32>,
    pub aspect: Updatable<f32>,
    znear: f32,
    zfar: f32,
    view_proj: Updatable<Matrix4<f32>>,
    frustum_planes: Updatable<[Plane; 6]>,
}

impl Camera {
    pub fn new(eye: Point3<f32>, aspect: f32) -> Camera {
        Self {
            eye: Updatable::new(eye),
            yaw: Updatable::new(0.0),
            pitch: Updatable::new(0.0),
            fovy: Updatable::new(70.0),
            aspect: Updatable::new(aspect),
            znear: 0.125,
            zfar: 500.0,
            view_proj: Updatable::new(Matrix4 {
                x: Vector4::new(0.0, 0.0, 0.0, 0.0),
                y: Vector4::new(0.0, 0.0, 0.0, 0.0),
                z: Vector4::new(0.0, 0.0, 0.0, 0.0),
                w: Vector4::new(0.0, 0.0, 0.0, 0.0),
            }),
            frustum_planes: Updatable::new([Plane::zero(); 6]),
        }
    }

    pub fn eye(&self) -> &Point3<f32> {
        self.eye.current()
    }

    pub fn yaw(&self) -> f32 {
        *self.yaw.current()
    }

    pub fn pitch(&self) -> f32 {
        *self.pitch.current()
    }

    pub fn forward(&self) -> Vector3<f32> {
        let (sy, cy) = self.yaw.current().sin_cos();
        let (sp, cp) = self.pitch.current().sin_cos();

        Vector3::new(cy * cp, sp, sy * cp).normalize()
    }

    pub fn right(&self) -> Vector3<f32> {
        self.forward().cross(UP).normalize()
    }

    pub fn target(&self) -> Point3<f32> {
        *self.eye.current() + self.forward()
    }

    pub fn needs_new_view_proj(&self) -> bool {
        self.eye.has_changed() || self.yaw.has_changed() || self.pitch.has_changed() || self.aspect.has_changed()
    }

    pub fn set_view_proj(&mut self) {
        let view = Matrix4::look_at_rh(*self.eye.current(), self.target(), UP);
        let proj = cgmath::perspective(Deg(*self.fovy.current()), *self.aspect.current(), self.znear, self.zfar);
        self.view_proj.update(OPENGL_TO_WGPU_MATRIX * proj * view);
    }

    pub fn view_proj(&self) -> &Updatable<Matrix4<f32>> {
        &self.view_proj
    }

    pub fn set_position(&mut self, position: cgmath::Point3<f32>) {
        self.eye.update(position);
    }

    pub fn set_rotation(&mut self, rotation: (f32, f32)) {
        self.yaw.update(rotation.0);
        self.pitch.update(rotation.1);
    }

    pub fn get_rotation(&self) -> (f32, f32) {
        (*self.yaw.current(), *self.pitch.current())
    }

    // utiliser dans Camera update si view proj a changé
    pub fn set_frustum_planes(&mut self) {
        let m = *self.view_proj.current();
        let new = [
            Plane {
                normal: Vector3::new(m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0]),
                d: m[3][3] + m[3][0],
            }, // left
            Plane {
                normal: Vector3::new(m[0][3] - m[0][0], m[1][3] - m[1][0], m[2][3] - m[2][0]),
                d: m[3][3] - m[3][0],
            }, // right
            Plane {
                normal: Vector3::new(m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1]),
                d: m[3][3] + m[3][1],
            }, // bottom
            Plane {
                normal: Vector3::new(m[0][3] - m[0][1], m[1][3] - m[1][1], m[2][3] - m[2][1]),
                d: m[3][3] - m[3][1],
            }, // top
            Plane {
                normal: Vector3::new(m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2]),
                d: m[3][3] + m[3][2],
            }, // near
            Plane {
                normal: Vector3::new(m[0][3] - m[0][2], m[1][3] - m[1][2], m[2][3] - m[2][2]),
                d: m[3][3] - m[3][2],
            }, // far
        ]
        .map(|p| p.normalize());
        self.frustum_planes.update(new);
    }

    #[inline(never)]
    pub fn get_frustum_planes(&self) -> &[Plane; 6] {
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
        let up = right.cross(forward);

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

        let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);

        for c in [
            nc - rn - un,
            nc + rn - un,
            nc - rn + un,
            nc + rn + un,
            fc - rn2 - un2,
            fc + rn2 - un2,
            fc - rn2 + un2,
            fc + rn2 + un2,
        ] {
            min = Vector3::new(min.x.min(c.x), min.y.min(c.y), min.z.min(c.z));
            max = Vector3::new(max.x.max(c.x), max.y.max(c.y), max.z.max(c.z));
        }

        AABB { min, max }
    }
}
