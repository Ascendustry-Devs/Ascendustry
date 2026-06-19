use anyhow::Result;
use clap::Parser;
use network::DEFAULT_SERVER_ADDRESS;
use project_core::log_server;
use server::config::ServerConfig;
use server::server::Server;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from(DEFAULT_SERVER_ADDRESS))]
    address: String,
    #[arg(short = 'p', long, default_value = "world/world_multi.stf")]
    save_path: String,
    #[arg(long)]
    no_tui: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.no_tui {
        run_headless(&args).await
    } else {
        run_with_tui(&args).await
    }
}

async fn run_headless(args: &Args) -> Result<()> {
    log_server!("Serveur: lancement (mode headless).");

    let server = Server::new(&args.address, &args.save_path, None, ServerConfig::default()).await?;
    server.run().await
}

#[cfg(feature = "tui")]
async fn run_with_tui(args: &Args) -> Result<()> {
    use server::tui::TuiCommand;
    use std::sync::atomic::{AtomicBool, Ordering};

    log_server!("Serveur: lancement.");

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
                    let player_ids: Vec<u64> = state.lock().unwrap().players.iter().map(|p| p.id).collect();
                    if let Some(cmd) = TuiApp::handle_input(key, &mut app, &player_ids) {
                        command_tx.send(cmd).ok();
                    }
                }
            }
        }

        disable_raw_mode().unwrap();
        crossterm::execute!(std::io::stdout(), LeaveAlternateScreen).unwrap();
    });

    let server = Arc::new(Server::new(&args.address, &args.save_path, Some(bridge), ServerConfig::default()).await?);

    let srv = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = srv.run().await {
            log_server!("Serveur arrêté avec erreur : {}", e);
        }
    });

    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            TuiCommand::Shutdown => {
                log_server!("Arrêt du serveur: Sauvegarde en cours...");
                if let Err(e) = server.save().await {
                    log_server!("Échec de la sauvegarde : {}", e);
                }
                log_server!("Arrêt demandé par la TUI.");
                break;
            }
            TuiCommand::Save => {
                log_server!("Sauvegarde demandée par la TUI.");
                if let Err(e) = server.save().await {
                    log_server!("Échec de la sauvegarde : {}", e);
                }
            }
            TuiCommand::Kick(id) => {
                if let Some(player) = server.state.get_player(id).await {
                    log_server!("Kick du joueur {} ({}) demandé par le Serveur.", player.username, id);
                    server.kick_player(&id, "Kické par le Serveur").await;
                } else {
                    log_server!("Joueur {} non trouvé.", id);
                }
            }
            _ => {}
        }
    }

    stop.store(true, Ordering::Relaxed);
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    Ok(())
}

#[cfg(not(feature = "tui"))]
async fn run_with_tui(_args: &Args) -> Result<()> {
    anyhow::bail!("Le binaire a été compilé sans le support TUI.");
}
