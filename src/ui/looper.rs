use std::time::Instant;

use crate::app::Action;
use crate::looper::LooperState;
use crate::ui::indicator;

pub fn render(ui: &mut egui::Ui, looper: &mut LooperState) -> Option<Action> {
    // Long-press detection needs continuous frames, not just
    // input-triggered repaints.
    ui.ctx().request_repaint();

    let mut action = None;
    let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));

    egui::Frame::default()
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            // Fixed top row, so the gear doesn't move when the indicator
            // grows below (e.g. the progress bar appearing).
            ui.horizontal(|ui| {
                ui.label("Looper Pedal");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("⚙").clicked() {
                        action = Some(Action::OpenSettings);
                    }
                });
            });
            ui.add_space(8.0);

            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                // Button and spacebar feed one InputHandler (see
                // `LooperState::tick`), so they can't desync.
                let icon =
                    indicator::press_button_icon(looper.state(), looper.is_long_press_active());
                let button_response = ui.add_sized(
                    [90.0, 90.0],
                    egui::Button::new(egui::RichText::new(icon).size(32.0)).corner_radius(45),
                );

                // Latched, not `is_pointer_button_down_on()` - see
                // `button_held` in `LooperState`.
                if button_response.is_pointer_button_down_on() {
                    looper.set_button_held(true);
                }
                if !ui.input(|i| i.pointer.primary_down()) {
                    looper.set_button_held(false);
                }

                looper.tick(space_down, Instant::now());

                ui.add_space(4.0);
                let (duration_secs, progress_fraction) = looper.loop_duration_and_progress();
                indicator::state_indicator(ui, looper.state(), duration_secs, progress_fraction);
                ui.add_space(4.0);
                ui.label("Space or button: cycle record/loop/stop  |  hold ~2s: clear");
            });
        });

    action
}
