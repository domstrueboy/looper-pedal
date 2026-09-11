//! What the settings screen would show, without opening a window.
//!
//! The picker logic can only be unit-tested against the mock; this runs
//! it against whatever this machine actually has.
//!
//!     cargo run -p looper-app --example settings_preview

use looper_core::settings::SettingsState;
use looper_hal::{Backends, cpal_backend::default_backends};

fn main() {
    let backends = Backends::new(default_backends());
    let mut settings = SettingsState::new(None);
    settings.refresh(&backends);

    let ids: Vec<(String, String)> = settings
        .backends
        .iter()
        .map(|b| (b.id.clone(), b.label.clone()))
        .collect();
    for (id, label) in ids {
        settings.config.backend = id.clone();
        settings.refresh_for_selected_backend(&backends);
        println!("\n=== {label} ({id}) ===");
        println!("  inputs  {:?}", settings.inputs);
        println!("  outputs {:?}", settings.outputs);
        println!("  duplex  {}", settings.duplex);
        println!("  chosen  in='{}'", settings.config.device_name);
        println!("          out='{}'", settings.config.output_device());
        println!("  rates   {:?}", settings.sample_rates);
        println!("  in ch   {}", settings.input_channels);
    }
}
