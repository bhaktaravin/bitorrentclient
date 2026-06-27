#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod game;
mod ui; 

use ui::StrategoApp; 


fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Stratego")
            .with_inner_size([1100.0, 780.0])
            .with_min_inner_size([900.0, 660.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Stratego",
        native_options,
        Box::new(|cc| Ok(Box::new(StrategoApp::new(cc)))),
    )
    
}