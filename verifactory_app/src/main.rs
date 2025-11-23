mod gui;
use std::{fs::File, sync::Arc};

use eframe::NativeOptions;
use gui::MyApp;

pub fn main() -> Result<(), eframe::Error> {
    let file = File::create("debug.log").unwrap();
    tracing_subscriber::fmt().with_writer(Arc::new(file)).init();
    eframe::run_native(
        "VeriFactory",
        NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<MyApp>::default())
        }),
    )
}
