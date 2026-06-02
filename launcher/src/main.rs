mod app;
mod launch;

use app::LauncherApp;
use eframe::egui;
use launch::LaunchMode;
use std::sync::mpsc;
use tokio::runtime::Runtime;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 350.0])
            .with_resizable(false),
        ..Default::default()
    };

    let (tx, rx) = mpsc::channel::<LaunchMode>();
    let tx2 = tx.clone();

    eframe::run_native("Ascendustry", options, Box::new(move |_cc| Box::new(LauncherApp::new(tx2))))?;

    drop(tx);
    if let Ok(mode) = rx.recv() {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        launch::set_play_mode(&runtime, mode);
    }

    Ok(())
}
