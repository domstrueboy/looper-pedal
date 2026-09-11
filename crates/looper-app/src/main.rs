// No console window behind the app in a release build. Debug builds keep
// theirs, which is where the underrun warnings and stream errors go - so
// this is "dev mode only" with no runtime flag to pass.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;

use std::time::Instant;

use app::{App, Screen};
use looper_hal::{Backends, cpal_backend::default_backends};

/// Advances the looper, hands the frame to a screen renderer, and applies
/// the action it returns. This and `ui/` are the only framework-aware code
/// - swapping GUI library rewrites them, not the app.
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let action = match &mut self.screen {
            Screen::Settings(settings) => looper_ui_egui::settings::render(ui, settings, &self.backends),
            Screen::Looper(looper) => {
                // Long-press detection and the pre-roll countdown need
                // continuous frames, not just input-triggered repaints.
                ui.ctx().request_repaint();
                // Advanced before the frame is drawn rather than part-way
                // through drawing it, so `ui/` only ever reports intent
                // and every widget reads the same state: the button used
                // to show what the indicator below it had already moved
                // past, for one frame, whenever a pre-roll ran out.
                let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));
                looper.tick(space_down, Instant::now());
                looper_ui_egui::looper::render(ui, looper)
            }
        };

        if let Some(action) = action {
            self.apply(action);
        }
    }
}

/// Big enough for the taskbar to downscale cleanly; the executable's own
/// icon resource carries the other sizes (see `build.rs`).
const WINDOW_ICON_SIZE: u32 = 64;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 400.0])
            .with_icon(egui::IconData {
                rgba: icon::icon_rgba(WINDOW_ICON_SIZE),
                width: WINDOW_ICON_SIZE,
                height: WINDOW_ICON_SIZE,
            }),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(Backends::new(default_backends()))))),
    )
}
