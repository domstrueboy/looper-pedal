mod audio;
mod input;

use std::sync::Arc;
use std::time::Instant;

use audio::engine::{self, SharedControl};
use audio::state_machine::LoopStateMachine;
use input::{InputEvent, InputHandler};

struct LooperApp {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
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
                println!("State: {:?}", self.state_machine.state());
            }
            InputEvent::LongPressClear => {
                self.state_machine.clear();
                self.control.publish_state(self.state_machine.state());
                self.control.request_clear();
                println!("State: {:?} (cleared)", self.state_machine.state());
            }
            InputEvent::None => {}
        }

        ui.label("Looper Pedal");
        ui.label(format!("State: {:?}", self.state_machine.state()));
        ui.label("Space: cycle record/loop/stop  |  hold ~2s: clear");
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
                input_handler: InputHandler::new(),
                _input_stream,
                _output_stream,
            }))
        }),
    )
}
