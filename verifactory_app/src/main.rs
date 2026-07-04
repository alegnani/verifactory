mod gui;
use std::{fs::File, io::stdout, sync::Arc};

use eframe::NativeOptions;
use gui::MyApp;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn main() -> Result<(), eframe::Error> {
    let file = File::create("debug.log").unwrap();
    let writer = Arc::new(stdout()).and(file);
    let layer = fmt::layer()
        .with_span_events(FmtSpan::ACTIVE)
        .with_writer(writer);
    tracing_subscriber::registry()
        // .with(fmt::layer().with_writer(Arc::new(file)))
        .with(layer)
        .with(EnvFilter::from_env("VERIFACTORY_LOG"))
        .init();
    eframe::run_native(
        "VeriFactory",
        NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<MyApp>::default())
        }),
    )
}
