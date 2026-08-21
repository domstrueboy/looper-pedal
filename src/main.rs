mod audio;

use audio::engine;

struct LooperApp {
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl eframe::App for LooperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("Looper Pedal");
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
            let (_input_stream, _output_stream) = engine::build_passthrough_streams();
            Ok(Box::new(LooperApp {
                _input_stream,
                _output_stream,
            }))
        }),
    )
}
