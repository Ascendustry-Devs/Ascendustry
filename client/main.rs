use clap::Parser;
use network::DEFAULT_SERVER_ADDRESS;
use winit::event_loop::EventLoop;

use client::game::GameState;
use engine::core::application::App;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from(DEFAULT_SERVER_ADDRESS))]
    address: String,
    #[arg(short, long, default_value_t = String::from("Lambda Player"))]
    name: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();
    let event_loop = EventLoop::with_user_event().build().expect("Failed starting event loop");
    // event_loop.set_control_flow(ControlFlow::Poll);
    let player_id = args.name.as_bytes().iter().fold(0u64, |acc, &byte| acc ^ u64::from(byte));
    let game_state = GameState::new(args.address, &args.name, player_id);
    let mut app = App::new(game_state);

    event_loop.run_app(&mut app).expect("Failed starting app");
}
