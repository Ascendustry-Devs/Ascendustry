use crate::launch::LaunchMode;
use eframe::egui;
use network::DEFAULT_SERVER_ADDRESS;
use std::sync::mpsc;

pub struct LauncherApp {
    tx: mpsc::Sender<LaunchMode>,
    address: String,
    show_multi: bool,
    save_path: String,
    username: String,
}

impl LauncherApp {
    pub fn new(tx: mpsc::Sender<LaunchMode>) -> Self {
        Self {
            tx,
            address: DEFAULT_SERVER_ADDRESS.to_string(),
            show_multi: false,
            save_path: "world/world_solo.stf".to_string(),
            username: "Lambda Player".to_string(),
        }
    }
}

impl eframe::App for LauncherApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Effectuer la logique du launcher ici (s'il y en a)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .show(ui, |ui| {
                        ui.heading("Ascendustry");
                    });

                ui.add_space(30.0);

                if self.show_multi {
                    ui.label("Nom du joueur :");
                    ui.add_sized(
                        [250.0, 30.0],
                        egui::TextEdit::singleline(&mut self.username).hint_text("Lambda Player"),
                    );

                    ui.add_space(10.0);
                    ui.label("Adresse du serveur (ip:port) :");
                    let resp = ui.add_sized(
                        [250.0, 30.0],
                        egui::TextEdit::singleline(&mut self.address).hint_text(DEFAULT_SERVER_ADDRESS),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let addr = address_or_default(&self.address);
                        self.tx
                            .send(LaunchMode::Multiplayer {
                                address: addr,
                                username: self.username.clone(),
                            })
                            .ok();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    ui.add_space(20.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(250.0, 40.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if ui.add_sized([120.0, 40.0], egui::Button::new("Retour")).clicked() {
                                self.show_multi = false;
                            }
                            ui.add_space(10.0);
                            if ui.add_sized([120.0, 40.0], egui::Button::new("Lancer")).clicked() {
                                let addr = address_or_default(&self.address);
                                self.tx
                                    .send(LaunchMode::Multiplayer {
                                        address: addr,
                                        username: self.username.clone(),
                                    })
                                    .ok();
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        },
                    );
                } else {
                    ui.label("Nom du joueur :");
                    ui.add_sized(
                        [250.0, 30.0],
                        egui::TextEdit::singleline(&mut self.username).hint_text("Lambda Player"),
                    );

                    ui.add_space(10.0);
                    ui.label("Chemin de la sauvegarde :");
                    ui.add_sized(
                        [250.0, 30.0],
                        egui::TextEdit::singleline(&mut self.save_path).hint_text("world/world_1.stf"),
                    );

                    ui.add_space(10.0);

                    if ui.add_sized([250.0, 50.0], egui::Button::new("Solo")).clicked() {
                        let path = save_path_or_default(&self.save_path);
                        self.tx
                            .send(LaunchMode::Singleplayer {
                                save_path: path,
                                username: self.username.clone(),
                            })
                            .ok();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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

fn save_path_or_default(path: &str) -> String {
    if path.trim().is_empty() {
        "world/world_solo.stf".to_string()
    } else {
        path.trim().to_string()
    }
}
