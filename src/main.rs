mod audio;

use std::sync::Arc;

use audio::engine::{self, SharedControl};
use audio::state_machine::LoopStateMachine;

struct LooperApp {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl eframe::App for LooperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Temporary input for step 6 verification only - Space cycles the
        // state machine, C clears. Step 7 replaces this with real
        // spacebar + long-press-to-clear detection.
        let (space_pressed, clear_pressed) = ui
            .ctx()
            .input(|i| (i.key_pressed(egui::Key::Space), i.key_pressed(egui::Key::C)));

        if space_pressed {
            self.state_machine.press();
            self.control.publish_state(self.state_machine.state());
            println!("State: {:?}", self.state_machine.state());
        }

        if clear_pressed {
            self.state_machine.clear();
            self.control.publish_state(self.state_machine.state());
            self.control.request_clear();
            println!("State: {:?} (cleared)", self.state_machine.state());
        }

        ui.label("Looper Pedal");
        ui.label(format!("State: {:?}", self.state_machine.state()));
        ui.label("Space: cycle record/loop/stop  |  C: clear (temporary, step 6 only)");
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
            let (_input_stream, _output_stream) = engine::build_looper_streams(Arc::clone(&control));
            Ok(Box::new(LooperApp {
                control,
                state_machine: LoopStateMachine::new(),
                _input_stream,
                _output_stream,
            }))
        }),
    )
}
