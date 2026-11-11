#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod covers;
mod diagnostics;
mod emulator;
mod glass;
mod library;
mod model;
mod rom;
mod scan;
mod sources;
mod store;
mod style;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([780.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RustyShelf",
        options,
        Box::new(|cc| Ok(Box::new(app::RustyShelfApp::new(cc)))),
    )
}
