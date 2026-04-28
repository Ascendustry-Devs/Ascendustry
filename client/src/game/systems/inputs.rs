use std::{collections::HashMap, mem};

use winit::keyboard::KeyCode;

pub struct InputState {
    mouse_delta: (f64, f64),
    pressed_keys: HashMap<KeyCode, bool>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_delta: (0.0, 0.0),
            pressed_keys: HashMap::new(),
        }
    }

    pub fn set_key_press(&mut self, key: KeyCode, is_pressed: bool) {
        self.pressed_keys.insert(key, is_pressed);
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        return *self.pressed_keys.get(&key).unwrap_or(&false);
    }

    pub fn set_mouse_delta(&mut self, delta: (f64, f64)) {
        self.mouse_delta = delta;
    }

    pub fn get_mouse_delta(&self) -> (f64, f64) {
        return self.mouse_delta;
    }

    pub fn take_mouse_delta(&mut self) -> (f64, f64) {
        mem::replace(&mut self.mouse_delta, (0.0, 0.0))
    }

    pub fn take_mouse_delta_f32(&mut self) -> (f32, f32) {
        let (dx, dy) = mem::replace(&mut self.mouse_delta, (0.0, 0.0));
        (dx as f32, dy as f32)
    }
}
