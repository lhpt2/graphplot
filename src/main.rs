mod app;
mod graph;
mod layout;

use app::GraphPlotApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Graphplot",
        native_options,
        Box::new(|_cc| Ok(Box::new(GraphPlotApp::default()))),
    )
}
