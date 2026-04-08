#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod keyboard;
mod midi_out;
mod sf2_engine;
mod midi_player;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SauPiano v0.01")
            .with_inner_size([1050.0, 580.0])
            .with_min_inner_size([800.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SauPiano v0.01",
        options,
        Box::new(|cc| Ok(Box::new(app::SauPianoApp::new(cc)))),
    )
}