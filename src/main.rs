mod app;
mod emulator;
mod model;
mod scan;
mod sources;
mod store;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
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
