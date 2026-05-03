mod common;
mod engine;
mod game;

use clap::Parser;
use std::net::Ipv4Addr;
use std::str::FromStr;
use winit::event_loop::EventLoop;

use crate::{engine::core::application::App, game::game::GameState};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("127.0.0.1:42677"))]
    address: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();
    let event_loop = EventLoop::with_user_event().build().expect("Failed starting event loop");
    let game_state = GameState::new(args.address);
    let mut app = App::new(game_state);

    event_loop.run_app(&mut app).expect("Failed starting app");
}
