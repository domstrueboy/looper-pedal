mod audio;
mod input;
mod ui;

use std::sync::Arc;
use std::time::Instant;

use audio::engine;
use audio::shared_control::SharedControl;
use audio::state_machine::LoopStateMachine;
use input::{InputEvent, InputHandler};

struct LooperApp {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
    sample_rate: u32,
    channels: u16,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl eframe::App for LooperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Long-press detection needs continuous frame updates, not just
        // repaints triggered by input events.
        ui.ctx().request_repaint();

        let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));
        match self.input_handler.update(space_down, Instant::now()) {
            InputEvent::ShortPress => {
                self.state_machine.press();
                self.control.publish_state(self.state_machine.state());
            }
            InputEvent::LongPressClear => {
                self.state_machine.clear();
                self.control.publish_state(self.state_machine.state());
                self.control.request_clear();
            }
            InputEvent::None => {}
        }

        let (duration_secs, progress_fraction) = self
            .control
            .loop_duration_and_progress(self.sample_rate, self.channels);

        egui::Frame::default()
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                ui.label("Looper Pedal");
                ui.add_space(4.0);
                ui::indicator::state_indicator(
                    ui,
                    self.state_machine.state(),
                    duration_secs,
                    progress_fraction,
                );
                ui.add_space(4.0);
                ui.label("Space: cycle record/loop/stop  |  hold ~2s: clear");
            });
    }
}

fn main() -> eframe::Result {
    engine::list_asio_devices();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| {
            let control = Arc::new(SharedControl::new());
            let (_input_stream, _output_stream, sample_rate, channels) =
                engine::build_looper_streams(Arc::clone(&control));
            Ok(Box::new(LooperApp {
                control,
                state_machine: LoopStateMachine::new(),
                input_handler: InputHandler::new(),
                sample_rate,
                channels,
                _input_stream,
                _output_stream,
            }))
        }),
    )
}
