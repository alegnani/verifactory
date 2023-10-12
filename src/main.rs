mod backends;
mod compiler;
mod entities;
mod gui;
mod import;
mod ir;
mod utils;

use std::{fs::File, sync::Arc};

use eframe::NativeOptions;
use gui::MyApp;

fn main() -> Result<(), eframe::Error> {
    let file = File::create("debug.log").unwrap();
    tracing_subscriber::fmt().with_writer(Arc::new(file)).init();
    eframe::run_native(
        "Factorio Verify",
        NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::<MyApp>::default()
        }),
    )
}
