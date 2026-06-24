use std::collections::HashMap;
use std::mem::replace;

use rustc_hash::{FxBuildHasher, FxHashMap};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

pub struct InputState {
    mouse_delta: (f64, f64),
    pressed_keys: FxHashMap<KeyCode, bool>,
    pressed_mouse_buttons: FxHashMap<MouseButton, bool>,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(unused)]
impl InputState {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            mouse_delta: (0.0, 0.0),
            pressed_keys: HashMap::with_hasher(FxBuildHasher),
            pressed_mouse_buttons: HashMap::with_hasher(FxBuildHasher),
        }
    }

    #[inline(always)]
    pub fn set_key_press(&mut self, key: KeyCode, is_pressed: bool) {
        self.pressed_keys.insert(key, is_pressed);
    }

    #[inline(always)]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        *self.pressed_keys.get(&key).unwrap_or(&false)
    }

    #[inline(always)]
    pub fn take_key_pressed(&mut self, key: KeyCode) -> bool {
        self.pressed_keys.remove(&key).unwrap_or(false)
    }

    #[inline(always)]
    pub fn set_mouse_button_press(&mut self, button: MouseButton, is_pressed: bool) {
        self.pressed_mouse_buttons.insert(button, is_pressed);
    }

    #[inline(always)]
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        *self.pressed_mouse_buttons.get(&button).unwrap_or(&false)
    }

    #[inline(always)]
    pub fn take_mouse_button_pressed(&mut self, button: MouseButton) -> bool {
        self.pressed_mouse_buttons.remove(&button).unwrap_or(false)
    }

    #[inline(always)]
    pub fn set_mouse_delta(&mut self, delta: (f64, f64)) {
        self.mouse_delta.0 += delta.0;
        self.mouse_delta.1 += delta.1;
    }

    #[inline(always)]
    pub const fn get_mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    #[inline(always)]
    pub const fn take_mouse_delta(&mut self) -> (f64, f64) {
        replace(&mut self.mouse_delta, (0.0, 0.0))
    }

    #[inline(always)]
    pub const fn take_mouse_delta_f32(&mut self) -> (f32, f32) {
        let (dx, dy) = self.take_mouse_delta();
        (dx as f32, dy as f32)
    }
}
