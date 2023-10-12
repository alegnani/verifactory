use std::path::Path;

use egui::{Ui, Window};
use egui_file::FileDialog;

use super::app::MyApp;

#[derive(Default)]
pub struct BlueprintString {
    open: bool,
    blueprint: String,
    should_load: bool,
}

impl BlueprintString {
    pub fn show(&mut self, ui: &mut Ui) {
        Window::new("Enter blueprint string")
            .open(&mut self.open)
            .show(ui.ctx(), |ui| {
                ui.text_edit_singleline(&mut self.blueprint);
                if ui.button("Load").clicked() {
                    self.should_load = true;
                }
            });
    }
}

impl MyApp {
    pub fn draw_menu(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("").show(ctx, |ui| {
            self.blueprint_string.show(ui);
            if self.blueprint_string.should_load {
                let blueprint = self.blueprint_string.blueprint.clone();
                self.load_string(&blueprint);
                self.blueprint_string.should_load = false;
                self.blueprint_string.open = false;
            }

            egui::menu::bar(ui, |ui| {
                /* File submenu */
                ui.menu_button("File", |ui| {
                    /* Open blueprint button, opens file dialog */
                    if ui.button("Open blueprint file").clicked() {
                        ui.close_menu();
                        let mut dialog =
                            FileDialog::open_file(self.open_file_state.opened_file.clone());
                        dialog.open();
                        self.open_file_state.open_file_dialog = Some(dialog);
                    }
                    if ui.button("Open blueprint").clicked() {
                        ui.close_menu();
                        self.blueprint_string = BlueprintString {
                            open: true,
                            should_load: false,
                            blueprint: String::new(),
                        };
                    }
                    /* Close button, terminates the application */
                    if ui.button("Close").clicked() {
                        std::process::exit(0);
                    }
                });
                /* Handle the "Open blueprint" dialog */
                let dialog = &mut self.open_file_state.open_file_dialog;
                let path = dialog.as_mut().and_then(|d| {
                    if d.show(ctx).selected() {
                        d.path().map(Path::to_path_buf)
                    } else {
                        None
                    }
                });
                if let Some(path) = path {
                    self.load_file(path);
                }
                /* View submenu */
                /* TODO */
                ui.menu_button("View", |ui| {});
            })
        });
    }
}
