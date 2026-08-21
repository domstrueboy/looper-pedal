use cpal::traits::HostTrait;

fn list_asio_devices() {
    let host = cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable");

    println!("ASIO devices:");
    for device in host.devices().expect("failed to enumerate ASIO devices") {
        println!("  - {device}");
    }
}

struct LooperApp;

impl eframe::App for LooperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("Looper Pedal");
    }
}

fn main() -> eframe::Result {
    list_asio_devices();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| Ok(Box::new(LooperApp))),
    )
}
