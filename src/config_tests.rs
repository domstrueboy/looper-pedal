use super::*;

fn saved() -> AppConfig {
    AppConfig {
        device_name: "Audient USB Audio ASIO Driver".to_string(),
        sample_rate: 48_000,
        input_channel: 1,
        volume_pct: 140,
        preroll_ms: 2_500,
        latency_ms: 12,
        max_loop_secs: 90,
        max_layers: 6,
        long_press_ms: 1_500,
    }
}

#[test]
fn a_round_trip_keeps_every_setting() {
    let text = saved().to_toml().expect("serializes");
    let read = AppConfig::from_toml(&text).expect("parses back");

    assert_eq!(read.device_name, "Audient USB Audio ASIO Driver");
    assert_eq!(read.sample_rate, 48_000);
    assert_eq!(read.input_channel, 1);
    assert_eq!(read.volume_pct, 140);
    assert_eq!(read.preroll_ms, 2_500);
    assert_eq!(read.latency_ms, 12);
    assert_eq!(read.max_loop_secs, 90);
    assert_eq!(read.max_layers, 6);
    assert_eq!(read.long_press_ms, 1_500);
}

#[test]
fn a_device_name_with_spaces_survives_being_written() {
    // A bare, unquoted value is not valid TOML, so this has to be
    // written quoted or the file won't read back.
    let text = saved().to_toml().expect("serializes");
    assert!(
        text.contains("\"Audient USB Audio ASIO Driver\""),
        "device name should be quoted, got: {text}"
    );
}

#[test]
fn settings_added_later_fall_back_to_their_defaults() {
    let config = AppConfig::from_toml("device_name = \"iD4\"\nsample_rate = 44100\n")
        .expect("a device and a rate are enough");

    assert_eq!(config.input_channel, 0);
    assert_eq!(config.volume_pct, DEFAULT_VOLUME_PCT);
    assert_eq!(config.preroll_ms, DEFAULT_PREROLL_MS);
    assert_eq!(config.latency_ms, DEFAULT_LATENCY_MS);
    assert_eq!(config.max_loop_secs, DEFAULT_MAX_LOOP_SECS);
    assert_eq!(config.max_layers, DEFAULT_MAX_LAYERS);
    assert_eq!(config.long_press_ms, DEFAULT_LONG_PRESS_MS);
}

#[test]
fn a_config_that_names_no_device_is_no_config() {
    assert!(AppConfig::from_toml("sample_rate = 44100\n").is_none());
    assert!(AppConfig::from_toml("device_name = \"iD4\"\n").is_none());
    assert!(AppConfig::from_toml("").is_none());
}

#[test]
fn unknown_keys_are_ignored() {
    let config = AppConfig::from_toml(
        "# hand-edited\ndevice_name = \"iD4\"\nsample_rate = 44100\nfuture_setting = 7\n",
    )
    .expect("an unrecognised key shouldn't invalidate the file");
    assert_eq!(config.sample_rate, 44100);
}

#[test]
fn a_file_that_is_not_toml_is_rejected_rather_than_guessed_at() {
    assert!(AppConfig::from_toml("device_name=iD4\nsample_rate=44100\n").is_none());
}

#[test]
fn a_hand_edited_file_is_held_to_the_supported_ranges() {
    // Nine hundred layers of a two-hour loop would try to allocate
    // terabytes before the window opened.
    let config = AppConfig::from_toml(
        "device_name = \"iD4\"\n\
         sample_rate = 44100\n\
         max_layers = 900\n\
         max_loop_secs = 7200\n\
         latency_ms = 0\n\
         volume_pct = 5000\n\
         long_press_ms = 10\n",
    )
    .expect("still loads, just not as written");

    assert_eq!(config.max_layers, *MAX_LAYERS_RANGE.end());
    assert_eq!(config.max_loop_secs, *MAX_LOOP_SECS_RANGE.end());
    assert_eq!(config.latency_ms, *LATENCY_MS_RANGE.start());
    assert_eq!(config.volume_pct, *VOLUME_PCT_RANGE.end());
    assert_eq!(config.long_press_ms, *LONG_PRESS_MS_RANGE.start());
}

#[test]
fn every_default_sits_inside_its_own_range() {
    assert!(VOLUME_PCT_RANGE.contains(&DEFAULT_VOLUME_PCT));
    assert!(PREROLL_MS_RANGE.contains(&DEFAULT_PREROLL_MS));
    assert!(LATENCY_MS_RANGE.contains(&DEFAULT_LATENCY_MS));
    assert!(MAX_LOOP_SECS_RANGE.contains(&DEFAULT_MAX_LOOP_SECS));
    assert!(MAX_LAYERS_RANGE.contains(&DEFAULT_MAX_LAYERS));
    assert!(LONG_PRESS_MS_RANGE.contains(&DEFAULT_LONG_PRESS_MS));
}

#[test]
fn memory_is_the_product_of_length_layers_and_rate() {
    let config = saved();
    // 90s x 6 layers x 48000 x 4 bytes
    assert_eq!(config.loop_memory_bytes(), 90 * 6 * 48_000 * 4);
}

// --- The file half -----------------------------------------------------

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("looper-pedal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_saved_file_reads_back_as_the_same_settings() {
    let path = temp_dir("config-round-trip").join("config.toml");
    saved().save_to(&path).expect("writes");

    let read = AppConfig::load_from(&path).expect("reads back");
    assert_eq!(read.device_name, "Audient USB Audio ASIO Driver");
    assert_eq!(read.sample_rate, 48_000);
    assert_eq!(read.max_layers, 6);
}

#[test]
fn saving_creates_the_directory_it_needs() {
    // A first run has no per-user config directory yet, so `save_to`
    // has to make one rather than failing on the missing parent.
    let path = temp_dir("config-makes-dir").join("config.toml");
    assert!(!path.parent().expect("has a parent").exists());

    saved().save_to(&path).expect("writes");
    assert!(path.exists());
}

#[test]
fn a_file_that_is_not_there_is_no_config() {
    let path = temp_dir("config-absent").join("config.toml");
    assert!(AppConfig::load_from(&path).is_none());
}
