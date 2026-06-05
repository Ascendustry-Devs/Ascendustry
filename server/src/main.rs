use anyhow::Result;
use clap::Parser;
use network::DEFAULT_SERVER_ADDRESS;
use project_core::log_server;
use server::server::Server;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from(DEFAULT_SERVER_ADDRESS))]
    address: String,
    #[arg(short = 'p', long, default_value = "world/world_1.stf")]
    save_path: String,
}

#[cfg(feature = "tui")]
#[tokio::main]
async fn main() -> Result<()> {
    log_server!("Serveur: lancement.");

    let args = Args::parse();

    let state = Arc::new(std::sync::Mutex::new(server::tui::bridge::TuiState::default()));
    let (command_tx, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
    let bridge = server::tui::bridge::TuiBridge::new(Arc::clone(&state), command_tx.clone());
    bridge.set_address(&args.address);

    server::tui::log::init_logging(Arc::clone(&state));

    let stop = Arc::new(AtomicBool::new(false));
    let tui_stop = Arc::clone(&stop);

    std::thread::spawn(move || {
        use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
        use ratatui::backend::CrosstermBackend;
        use ratatui::Terminal;
        use server::tui::app::TuiApp;

        enable_raw_mode().unwrap();
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen).unwrap();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut app = TuiApp::new();

        while !tui_stop.load(Ordering::Relaxed) {
            terminal
                .draw(|frame| {
                    let s = state.lock().unwrap();
                    TuiApp::draw(frame, &s, &app);
                })
                .unwrap();

            if crossterm::event::poll(std::time::Duration::from_millis(200)).unwrap() {
                if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                    if let Some(cmd) = TuiApp::handle_input(key, &mut app) {
                        command_tx.send(cmd).ok();
                    }
                }
            }
        }

        disable_raw_mode().unwrap();
        crossterm::execute!(std::io::stdout(), LeaveAlternateScreen).unwrap();
    });

    let server = Server::new(&args.address, &args.save_path, Some(bridge)).await?;

    loop {
        tokio::select! {
            _ = server.run() => break,
            Some(cmd) = command_rx.recv() => {
                match cmd {
                    server::tui::bridge::TuiCommand::Shutdown => {
                        log_server!("Arrêt demandé par la TUI.");
                        break;
                    },
                    server::tui::bridge::TuiCommand::Save => {
                        log_server!("Sauvegarde demandée par la TUI.");
                        if let Err(e) = server.save() {
                            log_server!("Échec de la sauvegarde : {}", e);
                        }
                    },
                    server::tui::bridge::TuiCommand::Kick(id) => {
                        log_server!("Kick du joueur {} demandé par la TUI (non implémenté).", id);
                        if let Some(player) = server.state.get_player(id) {
                            // player.kick();
                        }
                    },
                    _ => {},
                }
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    Ok(())
}
