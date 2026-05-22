use cgmath::{InnerSpace, Vector3};
use satiscore::constants::DECEL_COEF;
use winit::keyboard::KeyCode;

use crate::{
    physics::body::PhysicsBody,
    player::{camera::Camera, controllers::PlayerController},
    systems::inputs::InputState,
};

/// Contrôleur de déplacement au sol avec physique (gravité, collision, saut).
/// Lire WASD pour la direction horizontale, Space pour sauter.
/// La vélocité est appliquée au `PhysicsBody`, c'est `resolve_collision` qui
/// traduit la vélocité en déplacement et gère les collisions.
pub struct WalkPlayerController;

impl PlayerController for WalkPlayerController {
    fn update(&self, dt: f32, inputs: &mut InputState, body: &mut PhysicsBody, camera: &Camera) {
        // Direction horizontale depuis la caméra (forward/right projeté sur XZ)
        let forward = camera.forward();
        let right = camera.right();

        let forward_xz = Vector3::new(forward.x, 0.0, forward.z).normalize();
        let right_xz = Vector3::new(right.x, 0.0, right.z).normalize();

        let mut direction = Vector3::new(0.0, 0.0, 0.0);

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
        if direction.magnitude2() > 0.0 {
            let dir = direction.normalize();
            body.velocity.x = dir.x * body.walk_speed;
            body.velocity.z = dir.z * body.walk_speed;
        } else {
            // La vitesse du joueur s'estompe au lieu de se réinitialiser nette (impression réaliste de s'arrêter)
            let decel = DECEL_COEF.powf(dt);
            body.velocity.x *= decel;
            body.velocity.z *= decel;
        }

        // Saut : seulement si au sol
        if inputs.is_key_pressed(KeyCode::Space) && body.on_ground {
            body.velocity.y = body.jump_speed;
            body.on_ground = false;
        }
    }
}
