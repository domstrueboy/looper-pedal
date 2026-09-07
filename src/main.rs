// No console window behind the app in a release build. Debug builds keep
// theirs, which is where the underrun warnings and stream errors go - so
// this is "dev mode only" with no runtime flag to pass.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod config;
mod input;
mod looper;
mod settings;
mod ui;

use app::{App, Screen};

/// Hands each frame to a screen renderer and applies the action it returns.
/// This and `ui/` are the only framework-aware code - swapping GUI library
/// rewrites them, not the app.
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let action = match &mut self.screen {
            Screen::Settings(settings) => ui::settings::render(ui, settings),
            Screen::Looper(looper) => ui::looper::render(ui, looper),
        };

        if let Some(action) = action {
            self.apply(action);
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 400.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
