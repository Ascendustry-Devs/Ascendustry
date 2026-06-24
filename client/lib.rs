pub mod api;
pub mod game;
pub mod network;
pub mod player;
pub mod render;
pub mod systems;
pub mod ui;
pub mod world;

use winit::event_loop::EventLoop;

use crate::game::GameState;
use engine::core::application::App;

pub fn run_client(address: &str, player_name: &str) {
    env_logger::init();
    let player_id = player_name.as_bytes().iter().fold(0u64, |acc, &byte| acc ^ (byte as u64));

    let event_loop = EventLoop::with_user_event().build().expect("Failed starting event loop");
    let game_state = GameState::new(address.to_string(), player_name, player_id);
    let mut app = App::new(game_state);

    event_loop.run_app(&mut app).expect("Failed starting app");
}
