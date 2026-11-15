//! Точка входа для SDA GUI (egui + eframe).
//! Запуск: cargo run --bin sda-gui --features gui --release

use eframe::egui;
use sda_core::gui::app::SdaApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("SDA — Super Download Agent"),
        ..Default::default()
    };

    eframe::run_native(
        "SDA — Super Download Agent",
        options,
        Box::new(|cc| Box::new(SdaApp::new(cc))),
    )
}
