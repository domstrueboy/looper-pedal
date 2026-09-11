//! What this machine's audio backends actually report.
//!
//! Run against a real interface before wiring a backend into the app:
//! it is the cheapest way to see how a device names itself, which
//! directions it claims, and which of the rates we offer it will take -
//! none of which can be checked without the hardware in front of you.
//!
//!     cargo run -p looper-hal --features asio --example list_devices

use looper_hal::{Backends, cpal_backend::default_backends};

fn main() {
    let backends = Backends::new(default_backends());
    if backends.is_empty() {
        println!("No backends in this build. Try --features asio.");
        return;
    }

    for backend in backends.iter() {
        println!("\n=== {} ({}) ===", backend.label(), backend.id());
        let devices = match backend.devices() {
            Ok(devices) => devices,
            // Expected rather than exceptional: a machine with no ASIO
            // driver installed says so here.
            Err(err) => {
                println!("  unavailable: {err}");
                continue;
            }
        };
        if devices.is_empty() {
            println!("  no devices");
        }
        for device in devices {
            print!("  {:?}  {}", device.direction, device.id.name);
            match backend.caps(&device) {
                Ok(caps) => println!(
                    "\n      in {} ch, out {} ch, rates {:?}",
                    caps.input_channels, caps.output_channels, caps.sample_rates
                ),
                Err(err) => println!("\n      caps unavailable: {err}"),
            }
        }
    }
}
