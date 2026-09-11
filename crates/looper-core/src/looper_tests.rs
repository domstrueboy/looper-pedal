use super::*;

use looper_hal::mock::{self, MockDriver, mock};

/// Small enough to follow by hand, and a rate the mock device offers.
const RATE: u32 = 44_100;
const PREROLL_MS: u32 = 1_000;

fn settings() -> AppConfig {
    AppConfig {
        device_name: mock::DEVICE.to_string(),
        sample_rate: RATE,
        preroll_ms: PREROLL_MS,
        // Short enough that a test can hold past it deliberately.
        long_press_ms: 500,
        ..AppConfig::default()
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("looper-pedal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A looper on a device that isn't there, so nothing this does can reach
/// an interface or the loop of whoever is running the tests.
fn start(name: &str, settings: &AppConfig) -> (LooperState, MockDriver, PathBuf) {
    let (backend, driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);
    let dir = temp_dir(name);
    let looper =
        LooperState::start_in(&backends, settings, dir.clone()).expect("the mock device opens");
    (looper, driver, dir)
}

/// One press: down on a frame, up on the next. `ShortPress` is a release,
/// so it takes both.
fn press(looper: &mut LooperState, down: Instant, up: Instant) {
    looper.tick(true, down);
    looper.tick(false, up);
}

#[test]
fn starting_opens_a_stream_and_dropping_it_stops() {
    let (looper, driver, dir) = start("looper-open", &settings());
    assert!(driver.is_running(), "the device is open");
    assert_eq!(looper.state(), LoopState::Idle);

    drop(looper);

    assert!(!driver.is_running(), "and closes with the screen");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_device_that_is_not_there_says_so_instead_of_panicking() {
    let (backend, _driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);
    let missing = AppConfig {
        device_name: "Some Interface That Went Away".to_string(),
        ..settings()
    };

    // `LooperState` holds a live stream, so it is deliberately not
    // `Debug` - which rules out `expect_err` here.
    match LooperState::start_in(&backends, &missing, temp_dir("looper-missing")) {
        Err(err) => assert!(err.contains("not found"), "got: {err}"),
        Ok(_) => panic!("there is no such device"),
    }
}

#[test]
fn the_rate_it_reports_is_the_one_the_device_granted() {
    // Not the one that was asked for. Everything measured in samples -
    // the loop's length in seconds included - is wrong otherwise.
    let (backend, driver) = mock();
    let backends = Backends::new(vec![Box::new(backend.always_grants(48_000))]);
    let dir = temp_dir("looper-granted");
    let asked = AppConfig {
        sample_rate: 96_000,
        ..settings()
    };

    let mut looper = LooperState::start_in(&backends, &asked, dir.clone()).expect("opens");
    let start = Instant::now();
    press(&mut looper, start, start);
    assert_eq!(looper.state(), LoopState::Arming);

    // One second of a 48 kHz device, recorded after the pre-roll.
    looper.tick(false, start + Duration::from_millis(PREROLL_MS as u64));
    assert_eq!(looper.state(), LoopState::Recording);
    driver.cycle(&vec![0.0; 48_000]);
    let at = start + Duration::from_millis(PREROLL_MS as u64 + 1);
    press(&mut looper, at, at);

    looper.tick(false, at + Duration::from_millis(1));
    let (seconds, _) = looper.loop_duration_and_progress();
    assert!(
        (seconds - 1.0).abs() < 0.01,
        "a second at the granted rate, got {seconds}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- The pre-roll race -------------------------------------------------
//
// `tick` takes the input before it takes the expiry, so that a press
// meant to call the countdown off still wins when it lands on the very
// frame the countdown runs out. Until the looper could be built without
// a device, that rule was a comment.

#[test]
fn a_press_on_the_frame_the_pre_roll_expires_calls_it_off() {
    let (mut looper, _driver, dir) = start("looper-race-press", &settings());
    let start = Instant::now();

    press(&mut looper, start, start);
    assert_eq!(looper.state(), LoopState::Arming, "counting down");

    // Down before the countdown runs out, up exactly as it does.
    let expiry = start + Duration::from_millis(PREROLL_MS as u64);
    looper.tick(true, expiry - Duration::from_millis(1));
    looper.tick(false, expiry);

    assert_eq!(
        looper.state(),
        LoopState::Idle,
        "the press called it off; recording anyway would start a take nobody asked for"
    );

    // And it stays off - a cancelled countdown must not fire later.
    looper.tick(false, expiry + Duration::from_millis(500));
    assert_eq!(looper.state(), LoopState::Idle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pre_roll_left_alone_starts_recording_when_it_runs_out() {
    // The other side of the same rule: without a press, the expiry is
    // what has the say.
    let (mut looper, _driver, dir) = start("looper-race-alone", &settings());
    let start = Instant::now();

    press(&mut looper, start, start);
    let expiry = start + Duration::from_millis(PREROLL_MS as u64);

    looper.tick(false, expiry - Duration::from_millis(1));
    assert_eq!(looper.state(), LoopState::Arming, "not yet");

    looper.tick(false, expiry);
    assert_eq!(looper.state(), LoopState::Recording);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_countdown_readout_matches_the_frame_it_was_ticked_with() {
    // The readouts answer from the frame's own clock rather than asking
    // the time again, so a screen can't show a countdown that disagrees
    // with the state drawn beside it.
    let (mut looper, _driver, dir) = start("looper-readout", &settings());
    let start = Instant::now();
    press(&mut looper, start, start);

    looper.tick(false, start + Duration::from_millis(250));
    assert!((looper.preroll_remaining_secs() - 0.75).abs() < 1e-6);
    assert!((looper.preroll_progress() - 0.25).abs() < 1e-6);

    // Asked again with no tick between, it must not have moved.
    assert!((looper.preroll_progress() - 0.25).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(&dir);
}

// --- The press cycle, with samples actually flowing ---------------------

#[test]
fn a_take_records_loops_and_clears() {
    let no_preroll = AppConfig {
        preroll_ms: 0,
        ..settings()
    };
    let (mut looper, driver, dir) = start("looper-cycle", &no_preroll);
    let mut at = Instant::now();
    let mut step = |looper: &mut LooperState| {
        at += Duration::from_millis(10);
        press(looper, at, at);
    };

    step(&mut looper);
    assert_eq!(looper.state(), LoopState::Recording, "no pre-roll, straight in");

    driver.cycle(&vec![0.5; 2_048]);
    step(&mut looper);
    assert_eq!(looper.state(), LoopState::Looping);

    // The audio thread needs a callback to publish what it settled on.
    driver.cycle(&vec![0.0; 512]);
    at += Duration::from_millis(10);
    looper.tick(false, at);
    assert_eq!(looper.layer_count(), 1, "one layer, from one take");

    // Held past the threshold: a long press clears rather than cycling.
    looper.tick(true, at);
    looper.tick(true, at + Duration::from_millis(600));
    assert_eq!(looper.state(), LoopState::Idle, "cleared");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_saved_loop_comes_back_stopped_rather_than_playing() {
    let settings = settings();
    let (first, driver, dir) = start("looper-restore", &settings);
    let mut looper = first;
    let at = Instant::now();

    // Record something and let it be written.
    press(&mut looper, at, at);
    looper.tick(false, at + Duration::from_millis(PREROLL_MS as u64));
    driver.cycle(&vec![0.25; 4_096]);
    let stop = at + Duration::from_millis(PREROLL_MS as u64 + 10);
    press(&mut looper, stop, stop);
    driver.cycle(&vec![0.0; 512]);
    looper.tick(false, stop + Duration::from_millis(10));
    drop(looper);

    // Opening again on the same directory finds it.
    let (backend, driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);
    let restored = LooperState::start_in(&backends, &settings, dir.clone()).expect("opens");

    assert!(driver.is_running());
    assert_eq!(
        restored.state(),
        LoopState::Stopped,
        "there, but silent until it is asked for"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
