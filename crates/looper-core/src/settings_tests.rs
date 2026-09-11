use super::*;

use looper_hal::mock::{self, SPLIT_CAPTURE_RATE, mock, split};

use crate::audio::engine;

fn duplex_backends() -> Backends {
    Backends::new(vec![Box::new(mock().0)])
}

fn split_backends() -> Backends {
    Backends::new(vec![Box::new(split().0)])
}

fn refreshed(backends: &Backends) -> SettingsState {
    let mut settings = SettingsState::new(None);
    // `new` deliberately looks nothing up, so a caller can stop the
    // audio first. Without this the screen is empty.
    settings.refresh(backends);
    settings
}

#[test]
fn a_duplex_backend_needs_only_one_device_picked() {
    let settings = refreshed(&duplex_backends());

    assert!(settings.duplex);
    assert_eq!(settings.config.device_name, mock::DEVICE);
    assert_eq!(
        settings.config.output_device_name, None,
        "one device does both, so there is nothing to store"
    );
    assert_eq!(settings.config.output_device(), mock::DEVICE);
}

#[test]
fn a_split_backend_picks_both_ends() {
    let backends = split_backends();
    let settings = refreshed(&backends);

    assert!(!settings.duplex, "the two halves are separate devices");
    assert!(!settings.inputs.is_empty());
    assert!(!settings.outputs.is_empty());

    // The names match here, so nothing extra is stored - but they are
    // still two different devices, and what matters is that each end
    // resolves to the half that can actually do its job.
    let (input, output) =
        engine::resolve_devices(&backends, &settings.config).expect("both ends resolve");
    assert_eq!(input.direction, Direction::Input);
    assert_eq!(output.direction, Direction::Output);
}

#[test]
fn the_rates_offered_are_the_ones_both_ends_take() {
    // Not either end's own list. A shared-mode capture endpoint offers
    // only its mix rate while its render half claims a range it would
    // have to resample to reach - offering those would be offering a
    // rate the device never actually runs at.
    let settings = refreshed(&split_backends());

    assert_eq!(settings.sample_rates, vec![SPLIT_CAPTURE_RATE]);
    assert_eq!(settings.config.sample_rate, SPLIT_CAPTURE_RATE);
}

#[test]
fn a_duplex_backend_offers_everything_it_supports() {
    let settings = refreshed(&duplex_backends());

    assert!(settings.sample_rates.len() > 1, "nothing to intersect away");
    assert!(settings.sample_rates.contains(&44_100));
}

#[test]
fn a_backend_that_is_not_in_this_build_falls_back() {
    // An ASIO config carried to a machine with no driver: pick what is
    // there rather than leaving a name nothing matches.
    let backends = duplex_backends();
    let mut settings = SettingsState::new(None);
    settings.config.backend = "asio".to_string();
    settings.config.device_name = "Some Interface".to_string();

    settings.refresh(&backends);

    assert_eq!(settings.config.backend, "mock");
    assert_eq!(settings.config.device_name, mock::DEVICE);
}

#[test]
fn no_backends_at_all_leaves_nothing_selected() {
    let settings = refreshed(&Backends::new(Vec::new()));

    assert!(settings.backends.is_empty());
    assert!(settings.inputs.is_empty());
    assert!(settings.sample_rates.is_empty());
    assert_eq!(settings.input_channels, 0);
}

#[test]
fn an_input_channel_the_device_does_not_have_is_dropped() {
    let backends = duplex_backends();
    let mut settings = SettingsState::new(None);
    // The mock reports one input channel; a config asking for the fourth
    // came from a bigger interface that isn't plugged in now.
    settings.config.input_channel = 3;

    settings.refresh(&backends);

    assert_eq!(settings.config.input_channel, 0);
}

#[test]
fn switching_to_a_split_backend_stops_claiming_one_device_does_both() {
    // What changing the driver picker does. The stale `duplex` would
    // otherwise hide the output picker on a backend that needs it.
    let mut settings = refreshed(&duplex_backends());
    assert!(settings.duplex);

    let split = split_backends();
    settings.config.backend = "mock".to_string();
    settings.refresh_for_selected_backend(&split);

    assert!(!settings.duplex);
    let (_, output) =
        engine::resolve_devices(&split, &settings.config).expect("an output is resolvable");
    assert_eq!(output.direction, Direction::Output);
}
