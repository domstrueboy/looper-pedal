use std::time::Instant;

use crate::app::Action;
use crate::audio::loop_stack::MAX_LAYERS;
use crate::audio::state_machine::LoopState;
use crate::looper::LooperState;
use crate::ui::indicator;

/// Both secondary buttons share a size, so the row can be measured and
/// centered without laying it out twice.
const CONTROL_BUTTON_SIZE: [f32; 2] = [118.0, 24.0];

/// Gathered fresh each time it's needed: the values change as the frame
/// advances the looper.
fn readout(looper: &LooperState) -> indicator::Readout {
    let (loop_duration_secs, progress_fraction) = looper.loop_duration_and_progress();
    indicator::Readout {
        state: looper.state(),
        loop_duration_secs,
        progress_fraction,
        countdown_secs: looper.preroll_remaining_secs(),
        arming_fraction: looper.preroll_progress(),
    }
}

pub fn render(ui: &mut egui::Ui, looper: &mut LooperState) -> Option<Action> {
    // Long-press detection needs continuous frames, not just
    // input-triggered repaints.
    ui.ctx().request_repaint();

    let mut action = None;
    let (space_down, overdub_pressed, remove_pressed) = ui.ctx().input(|i| {
        (
            i.key_down(egui::Key::Space),
            // Edge-triggered, unlike the main control: neither of these
            // has a long-press meaning, so there's nothing to time.
            i.key_pressed(egui::Key::O),
            i.key_pressed(egui::Key::R),
        )
    });

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
                let label =
                    indicator::press_button_label(&readout(looper), looper.is_long_press_active());
                let button_response = ui.add_sized(
                    [90.0, 90.0],
                    egui::Button::new(egui::RichText::new(label).size(32.0)).corner_radius(45),
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
                indicator::state_indicator(ui, &readout(looper));

                // Allocated at exactly the row's own width so the
                // centering layout above can center it - a plain
                // `horizontal` would take the full width and sit left.
                let row = egui::vec2(
                    CONTROL_BUTTON_SIZE[0] * 2.0 + ui.spacing().item_spacing.x,
                    CONTROL_BUTTON_SIZE[1],
                );
                ui.allocate_ui_with_layout(
                    row,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let overdub_label = if looper.state() == LoopState::Overdubbing {
                            "Finish overdub"
                        } else {
                            "Overdub (O)"
                        };
                        let overdub_clicked = ui
                            .add_enabled_ui(looper.can_overdub(), |ui| {
                                ui.add_sized(CONTROL_BUTTON_SIZE, egui::Button::new(overdub_label))
                            })
                            .inner
                            .clicked();
                        if overdub_clicked || overdub_pressed {
                            looper.toggle_overdub();
                        }

                        let remove_clicked = ui
                            .add_enabled_ui(looper.can_remove_layer(), |ui| {
                                ui.add_sized(
                                    CONTROL_BUTTON_SIZE,
                                    egui::Button::new("Remove last (R)"),
                                )
                            })
                            .inner
                            .clicked();
                        if remove_clicked || remove_pressed {
                            looper.remove_last_layer();
                        }
                    },
                );

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Layers: {}/{}", looper.layer_count(), MAX_LAYERS),
                );

                // Kept to three short lines: one long one would wrap
                // raggedly at this window width.
                ui.add_space(4.0);
                ui.label("Space or button: record / loop / stop");
                ui.label("Hold either ~2s to clear");
                ui.label("O: overdub   |   R: remove last");
            });
        });

    action
}
