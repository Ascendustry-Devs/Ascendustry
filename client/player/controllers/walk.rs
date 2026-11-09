use crate::player::camera::Camera;
use crate::player::controllers::PlayerController;
use crate::systems::inputs::InputState;
use game::constants::DECEL_COEF;
use physics::body::PhysicsBody;
use project_core::general_types::vec3::Vec3;
use winit::keyboard::KeyCode;

/// Contrôleur de déplacement au sol avec physique (gravité, collision, saut).
///
/// Lire WASD pour la direction horizontale, Space pour sauter.
///
/// La vélocité est appliquée au `PhysicsBody`, c'est `resolve_collision` qui
/// traduit la vélocité en déplacement et gère les collisions.
pub struct WalkPlayerController;

impl PlayerController for WalkPlayerController {
    fn update(&self, dt: f32, inputs: &mut InputState, body: &mut PhysicsBody, camera: &Camera) {
        // Direction horizontale depuis la caméra (forward/right projeté sur XZ)
        let forward = camera.forward();
        let right = camera.right();

        let forward_xz = Vec3::new(*forward.x(), 0.0, *forward.z()).normalize();
        let right_xz = Vec3::new(*right.x(), 0.0, *right.z()).normalize();

        let mut direction = Vec3::new(0.0, 0.0, 0.0);

        if inputs.is_key_pressed(KeyCode::KeyW) {
            direction += forward_xz;
        }
        if inputs.is_key_pressed(KeyCode::KeyS) {
            direction -= forward_xz;
        }
        if inputs.is_key_pressed(KeyCode::KeyD) {
            direction += right_xz;
        }
        if inputs.is_key_pressed(KeyCode::KeyA) {
            direction -= right_xz;
        }

        // Appliquer la vélocité horizontale (walk_speed)
        let mut v = *body.velocity.current();
        if direction.sqr_magnitude() > 0.0 {
            let dir = direction.normalize();
            *v.x_mut() = dir.x() * body.walk_speed;
            *v.z_mut() = dir.z() * body.walk_speed;
        } else {
            // La vitesse du joueur s'estompe au lieu de se réinitialiser nette (impression réaliste de s'arrêter)
            let decel = DECEL_COEF.powf(dt);
            *v.x_mut() *= decel;
            *v.z_mut() *= decel;
        }

        // Saut : seulement si au sol
        if inputs.is_key_pressed(KeyCode::Space) && body.on_ground {
            *v.y_mut() = body.jump_speed;
            body.on_ground = false;
        }

        body.velocity.update(v);
    }
}
