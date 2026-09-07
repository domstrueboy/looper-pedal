mod app;
mod audio;
mod config;
mod input;
mod looper;
mod settings;
mod ui;

use app::{App, Screen};

/// The only place that knows eframe/egui exists at the app level: it hands
/// the frame to whichever screen's renderer and applies the action that
/// comes back. Everything it drives is framework-free (see `app.rs`), so
/// swapping egui for another native GUI library means rewriting this file
/// and `ui/`, not the app itself.
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
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 300.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
