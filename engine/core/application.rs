use core::*;
use std::mem;
use std::process::exit;
use std::sync::Arc;

use project_core::{log_client, log_err_client};
use winit::event_loop::ActiveEventLoop;
use winit::{application::ApplicationHandler, keyboard::KeyCode, keyboard::PhysicalKey};

use crate::audio::GameAudioManager;
use crate::core::frame::{EngineFrameData, GameFrameData};
use crate::core::state::State;
use crate::render::render::Renderer;
use winit::event::{DeviceEvent, DeviceId, KeyEvent, WindowEvent};
use winit::window::{CursorGrabMode, Window};

#[allow(unused)]
pub enum AppEvent {
    None,
}

impl AppEvent {
    #[inline(always)]
    pub fn to_string(&self) -> String {
        match self {
            AppEvent::None => "None".to_string(),
        }
    }
}

pub trait AppState {
    fn init(&mut self, renderer: &mut Renderer, audio_manager: &mut Option<GameAudioManager>);
    fn update(&mut self, frame: &EngineFrameData, data: &mut GameFrameData, renderer: &mut Renderer);
    fn on_mouse_move(&mut self, dx: f64, dy: f64);
    fn on_key(&mut self, code: KeyCode, is_pressed: bool);
    fn dispose(&mut self);
}

pub struct App<S: AppState> {
    engine_state: Option<State>,
    app_state: S,
}

impl<S: AppState> App<S> {
    pub fn new(app_state: S) -> Self {
        Self {
            engine_state: None,
            app_state,
        }
    }
}

impl<S: AppState> ApplicationHandler<AppEvent> for App<S> {
    #[inline(always)]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_maximized(true).with_title("Ascendustry");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let mut engine = pollster::block_on(State::new(window, &self.app_state)).unwrap();
        self.app_state.init(&mut engine.renderer, &mut engine.audio_manager);
        self.engine_state = Some(engine);
    }

    #[inline(always)]
    fn suspended(&mut self, _: &ActiveEventLoop) {
        if let Some(engine) = self.engine_state.as_mut() {
            engine.dispose();
        }
    }

    #[inline(always)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        log_client!("Évènement système reçu: {:?}", event.to_string());
    }

    #[inline(always)]
    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.app_state.on_mouse_move(delta.0, delta.1);
        }
    }

    #[inline(always)]
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        let Some(state) = self.engine_state.as_mut() else {
            return;
        };

        state.update();
        // state.game_frame_data.reset();

        self.app_state
            .update(&state.engine_frame_data, &mut state.game_frame_data, &mut state.renderer);

        mem::swap(
            &mut state.game_frame_data.visible_meshes,
            &mut state.renderer.render_manager.ids_to_render,
        );

        state.window.request_redraw();
    }

    #[inline(always)]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        let Some(state) = self.engine_state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::Focused(true) => {
                state.window.set_cursor_visible(false);
                state.window.set_cursor_grab(CursorGrabMode::Confined).unwrap_or(());
            }
            WindowEvent::Focused(false) => {
                state.window.set_cursor_visible(true);
                state.window.set_cursor_grab(CursorGrabMode::None).unwrap_or(());
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => match state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    let size: winit::dpi::PhysicalSize<u32> = state.window.inner_size();
                    state.resize(size.width, size.height);
                }
                Err(e) => {
                    log_err_client!("Unable to render.\nError: {}", e);
                }
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                if code == KeyCode::Escape && key_state.is_pressed() {
                    event_loop.exit();
                    return;
                } else if code == KeyCode::Digit1 && key_state.is_pressed() {
                    state.renderer.debug.wireframe = !state.renderer.debug.wireframe;
                    state.window.request_redraw();
                } else if code == KeyCode::Digit2 && key_state.is_pressed() {
                    state.renderer.debug.show_chunk_borders = !state.renderer.debug.show_chunk_borders;
                    state.window.request_redraw();
                } else {
                    self.app_state.on_key(code, key_state.is_pressed());
                }
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.app_state.dispose();
        if let Some(engine) = self.engine_state.as_mut() {
            engine.dispose();
        }
        log_client!("Exiting...");
        exit(0);
    }
}
