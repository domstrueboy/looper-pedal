//! Opens a device for real and runs it briefly, to prove `start` works.
//!
//! Enumeration can be checked from `list_devices`; actually claiming a
//! driver and having both callbacks fire cannot, and it is the half that
//! breaks. Output is silence on purpose - this reports what the driver
//! does, it isn't something to listen to.
//!
//!     cargo run -p looper-hal --features asio --example open_device

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use looper_hal::{
    Backends, InputProcessor, OutputProcessor, StreamRequest, cpal_backend::default_backends,
};

/// Counts calls and the largest buffer seen, which is the number that
/// says what the driver's period actually is.
#[derive(Default)]
struct Counter {
    calls: AtomicUsize,
    largest: AtomicUsize,
}

impl Counter {
    fn note(&self, frames: usize) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.largest.fetch_max(frames, Ordering::Relaxed);
    }

    fn report(&self, label: &str, channels: u16) {
        let calls = self.calls.load(Ordering::Relaxed);
        let largest = self.largest.load(Ordering::Relaxed) / channels.max(1) as usize;
        println!("  {label}: {calls} callbacks, largest {largest} frames");
    }
}

struct Capture(Arc<Counter>);

impl InputProcessor for Capture {
    fn process(&mut self, input: &[f32]) {
        self.0.note(input.len());
    }
}

struct Silence(Arc<Counter>);

impl OutputProcessor for Silence {
    fn process(&mut self, output: &mut [f32]) {
        self.0.note(output.len());
        output.fill(0.0);
    }
}

fn main() {
    let backends = Backends::new(default_backends());
    let Some(device) = backends
        .devices()
        .into_iter()
        .find(|device| device.direction.can_capture())
    else {
        println!("No capture device. Try --features asio.");
        return;
    };

    let caps = backends
        .get(device.id.backend)
        .expect("the backend that just listed it")
        .caps(&device.id)
        .expect("caps");
    let sample_rate = caps.sample_rates.first().copied().unwrap_or(48_000);

    println!("Opening '{}' at {sample_rate} Hz", device.id.name);
    let open = match backends.open(&StreamRequest {
        input: device.id.clone(),
        output: device.id.clone(),
        sample_rate,
    }) {
        Ok(open) => open,
        Err(err) => {
            println!("could not open: {err}");
            return;
        }
    };

    let format = open.format();
    println!(
        "Granted: {} Hz, {} in, {} out",
        format.sample_rate, format.input_channels, format.output_channels
    );

    let captured = Arc::new(Counter::default());
    let rendered = Arc::new(Counter::default());
    let errors = Arc::new(Counter::default());
    let sink = Arc::clone(&errors);

    let stream = match open.start(
        Box::new(Capture(Arc::clone(&captured))),
        Box::new(Silence(Arc::clone(&rendered))),
        Arc::new(move |err| {
            sink.note(0);
            eprintln!("  stream error: {err}");
        }),
    ) {
        Ok(stream) => stream,
        Err(err) => {
            println!("could not start: {err}");
            return;
        }
    };

    std::thread::sleep(std::time::Duration::from_secs(2));
    captured.report("capture", format.input_channels);
    rendered.report("render", format.output_channels);

    // Dropping it is what stops the audio, so say whether that worked
    // rather than leaving the driver claimed on the way out.
    drop(stream);
    println!("Stopped cleanly.");
}
