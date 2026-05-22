use crate::launch::LaunchMode;
use eframe::egui;
use satiscore::network::DEFAULT_SERVER_ADDRESS;
use std::sync::mpsc;

pub struct LauncherApp {
    tx: mpsc::Sender<LaunchMode>,
    address: String,
    show_multi: bool,
}

impl LauncherApp {
    pub fn new(tx: mpsc::Sender<LaunchMode>) -> Self {
        Self {
            tx,
            address: DEFAULT_SERVER_ADDRESS.to_string(),
            show_multi: false,
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(20.0, 10.0))
                    .show(ui, |ui| {
                        ui.heading("Satisfactorio");
                    });

                ui.add_space(30.0);

                if self.show_multi {
                    ui.label("Adresse du serveur (ip:port) :");
                    let resp = ui.add_sized(
                        [250.0, 30.0],
                        egui::TextEdit::singleline(&mut self.address).hint_text(DEFAULT_SERVER_ADDRESS),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let addr = address_or_default(&self.address);
                        self.tx.send(LaunchMode::Multiplayer(addr)).ok();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    ui.add_space(20.0);

                    ui.allocate_ui_with_layout(egui::vec2(250.0, 40.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if ui.add_sized([120.0, 40.0], egui::Button::new("Retour")).clicked() {
                            self.show_multi = false;
                        }
                        ui.add_space(10.0);
                        if ui.add_sized([120.0, 40.0], egui::Button::new("Lancer")).clicked() {
                            let addr = address_or_default(&self.address);
                            self.tx.send(LaunchMode::Multiplayer(addr)).ok();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                } else {
                    if ui.add_sized([250.0, 50.0], egui::Button::new("Solo")).clicked() {
                        self.tx.send(LaunchMode::Singleplayer).ok();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    ui.add_space(15.0);

                    if ui.add_sized([250.0, 50.0], egui::Button::new("Multijoueur")).clicked() {
                        self.show_multi = true;
                    }
                }
            });
        });
    }
}

fn address_or_default(addr: &str) -> String {
    if addr.trim().is_empty() {
        DEFAULT_SERVER_ADDRESS.to_string()
    } else {
        addr.trim().to_string()
    }
}
