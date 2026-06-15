// In player camera
pub fn get_near_to_far_vectors(&self) -> [Vector3<f32>; 4] {
    let fov = *self.fovy.current();
    let aspect = *self.aspect.current();
    let eye = *self.eye.current();
    let forward = self.forward();
    let right = self.right();
    let up = right.cross(forward);

    let tan_half_fov = (fov / 2.0_f32).to_radians().tan();
    //                      ^^^^^^^^^^^^^^^^^^^^^^^^^^
    // fovy est stocké en degrés dans ton TAD, il faut convertir

    let fh = tan_half_fov * self.zfar;
    let fw = fh * aspect;

    let fc = eye + forward * self.zfar;
    let r = right * fw;
    let u = up * fh;

    // top-left, top-right, bottom-left, bottom-right
    [
        (fc - r + u) - eye, // top-left
        (fc + r + u) - eye, // top-right
        (fc - r - u) - eye, // bottom-left
        (fc + r - u) - eye, // bottom-right
    ]
}

// In world
#[inline(never)]
fn cull_chunks_rasterizer(&self, out: &mut FxHashSet<u32>) {
    let cam = &self.player.state.camera;
    let eye = *cam.eye.current();
    let forward = cam.forward();
    let right = cam.right();
    let up = right.cross(forward);
    let fovy = *cam.fovy.current();
    let aspect = *cam.aspect.current();

    // 1. Axe dominant du forward
    let (fx, fy, fz) = forward.into();
    let (ax, ay, az) = (fx.abs(), fy.abs(), fz.abs());
    let (d_ax, u_ax, v_ax) = if ax >= ay && ax >= az {
        (0usize, 1usize, 2usize)
    } else if ay >= az {
        (1usize, 0usize, 2usize)
    } else {
        (2usize, 0usize, 1usize)
    };
    let sign_d = forward[d_ax].signum();

    // 2. Dimensions du frustum en blocs (monde)
    let tan_half = (fovy / 2.0).to_radians().tan();
    let nw = tan_half * cam.znear * aspect;
    let nh = tan_half * cam.znear;
    let fw = tan_half * cam.zfar * aspect;
    let fh = tan_half * cam.zfar;

    let nc = eye + forward * cam.znear;
    let fc = eye + forward * cam.zfar;
    let rn = right * nw;
    let rn2 = right * fw;
    let un = up * nh;
    let un2 = up * fh;

    // 3. 8 coins du frustum en espace chunk (division par CHUNK_SIZE_F)
    //    near/far sont des [x, y, z; 4] (BL, BR, TL, TR)
    let cs = CHUNK_SIZE_F;
    let near: [[f32; 3]; 4] = [
        [(nc - rn - un).x / cs, (nc - rn - un).y / cs, (nc - rn - un).z / cs],
        [(nc + rn - un).x / cs, (nc + rn - un).y / cs, (nc + rn - un).z / cs],
        [(nc - rn + un).x / cs, (nc - rn + un).y / cs, (nc - rn + un).z / cs],
        [(nc + rn + un).x / cs, (nc + rn + un).y / cs, (nc + rn + un).z / cs],
    ];
    let far: [[f32; 3]; 4] = [
        [(fc - rn2 - un2).x / cs, (fc - rn2 - un2).y / cs, (fc - rn2 - un2).z / cs],
        [(fc + rn2 - un2).x / cs, (fc + rn2 - un2).y / cs, (fc + rn2 - un2).z / cs],
        [(fc - rn2 + un2).x / cs, (fc - rn2 + un2).y / cs, (fc - rn2 + un2).z / cs],
        [(fc + rn2 + un2).x / cs, (fc + rn2 + un2).y / cs, (fc + rn2 + un2).z / cs],
    ];

    // 4. Rayons normalisés (delta_u / delta_d, delta_v / delta_d)
    //    Après normalisation : chaque rayon a une composante D = 1.0.
    //    rays_u[i] = déplacement en U par unité de D
    //    d_lo[i], d_hi[i] = intervalle de D valide pour ce rayon
    let mut rays_u = [0.0f32; 4];
    let mut rays_v = [0.0f32; 4];
    let mut d_lo = [0.0f32; 4];
    let mut d_hi = [0.0f32; 4];

    for i in 0..4 {
        let nd = near[i][d_ax];
        let fd = far[i][d_ax];
        let delta_d = fd - nd;
        if delta_d.abs() > 1e-8 {
            rays_u[i] = (far[i][u_ax] - near[i][u_ax]) / delta_d;
            rays_v[i] = (far[i][v_ax] - near[i][v_ax]) / delta_d;
            d_lo[i] = nd.min(fd);
            d_hi[i] = nd.max(fd);
        }
    }

    // 5. Drift max (pour l'expansion conservative entre tranches)
    let max_drift_u = rays_u.iter().map(|r| r.abs()).fold(0.0, f32::max);
    let max_drift_v = rays_v.iter().map(|r| r.abs()).fold(0.0, f32::max);

    // 6. Intervalle de tranches D (en coordonnées chunk)
    let origin_d = eye[d_ax] / cs;
    let start_d = (origin_d + sign_d * (cam.znear / cs)).floor() as i32;
    let end_d = (origin_d + sign_d * (cam.zfar / cs)).ceil() as i32;
    let step_dir = sign_d as i32;
    let max_iter = (end_d - start_d).abs() as i32 + 1;

    // 7. Marchage
    for iter in 0..max_iter {
        let slice_d = start_d + step_dir * iter;

        // Bbox flottante en UV (coordonnées chunk)
        let mut min_u_f = f32::MAX;
        let mut max_u_f = f32::MIN;
        let mut min_v_f = f32::MAX;
        let mut max_v_f = f32::MIN;
        let mut any_valid = false;

        for i in 0..4 {
            if d_hi[i] - d_lo[i] < 1e-8 {
                continue;
            }
            let sd = slice_d as f32;
            if sd < d_lo[i] || sd > d_hi[i] {
                continue;
            }
            any_valid = true;

            let delta_d = sd - near[i][d_ax];
            let ou = near[i][u_ax] + rays_u[i] * delta_d;
            let ov = near[i][v_ax] + rays_v[i] * delta_d;

            if ou < min_u_f {
                min_u_f = ou;
            }
            if ou > max_u_f {
                max_u_f = ou;
            }
            if ov < min_v_f {
                min_v_f = ov;
            }
            if ov > max_v_f {
                max_v_f = ov;
            }
        }

        if !any_valid {
            continue;
        }

        // Expansion conservative : le chunk en D=slice_d couvre [slice_d, slice_d+1).
        // L'UV du frustum y dérive de max_drift en chaque direction.
        let min_u = (min_u_f - max_drift_u).floor() as i32;
        let max_u = (max_u_f + max_drift_u).floor() as i32;
        let min_v = (min_v_f - max_drift_v).floor() as i32;
        let max_v = (max_v_f + max_drift_v).floor() as i32;

        for cu in min_u..=max_u {
            for cv in min_v..=max_v {
                let mut coords = [0i32; 3];
                coords[d_ax] = slice_d;
                coords[u_ax] = cu;
                coords[v_ax] = cv;

                let key = &(coords[0], coords[1], coords[2]);
                if let Some(mesh) = self.world_mesh.meshes.get(key) {
                    if let Some(mesh_id) = mesh.id {
                        out.insert(mesh_id);
                    }
                }
            }
        }
    }

    // 8. Chunk caméra (juste sous la caméra, avant le near plane)
    let cx = (eye.x / cs).floor() as i32;
    let cy = (eye.y / cs).floor() as i32;
    let cz = (eye.z / cs).floor() as i32;
    if let Some(mesh) = self.world_mesh.meshes.get(&(cx, cy, cz)) {
        if let Some(mesh_id) = mesh.id {
            out.insert(mesh_id);
        }
    }
}
