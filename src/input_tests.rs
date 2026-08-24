use super::*;

#[test]
fn press_then_quick_release_yields_short_press() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    assert_eq!(input.update(true, t0), InputEvent::None);
    assert_eq!(input.update(true, t0 + Duration::from_millis(100)), InputEvent::None);
    assert_eq!(
        input.update(false, t0 + Duration::from_millis(150)),
        InputEvent::ShortPress
    );
}

#[test]
fn holding_past_threshold_yields_long_press_clear_while_still_held() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    assert_eq!(input.update(true, t0), InputEvent::None);
    assert_eq!(
        input.update(true, t0 + Duration::from_millis(2001)),
        InputEvent::LongPressClear
    );
}

#[test]
fn releasing_after_long_press_does_not_also_fire_short_press() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    input.update(true, t0);
    assert_eq!(
        input.update(true, t0 + Duration::from_millis(2001)),
        InputEvent::LongPressClear
    );
    assert_eq!(
        input.update(false, t0 + Duration::from_millis(2100)),
        InputEvent::None
    );
}

#[test]
fn long_press_fires_only_once_per_hold() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    input.update(true, t0);
    assert_eq!(
        input.update(true, t0 + Duration::from_millis(2001)),
        InputEvent::LongPressClear
    );
    assert_eq!(
        input.update(true, t0 + Duration::from_millis(3000)),
        InputEvent::None
    );
}

#[test]
fn releasing_without_ever_pressing_yields_none() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();
    assert_eq!(input.update(false, t0), InputEvent::None);
}

#[test]
fn can_short_press_again_after_a_short_press() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    input.update(true, t0);
    assert_eq!(
        input.update(false, t0 + Duration::from_millis(100)),
        InputEvent::ShortPress
    );

    let t1 = t0 + Duration::from_millis(500);
    input.update(true, t1);
    assert_eq!(
        input.update(false, t1 + Duration::from_millis(100)),
        InputEvent::ShortPress
    );
}

#[test]
fn is_long_press_active_only_between_firing_and_release() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    input.update(true, t0);
    assert!(!input.is_long_press_active());

    input.update(true, t0 + Duration::from_millis(2001));
    assert!(input.is_long_press_active());

    input.update(true, t0 + Duration::from_millis(2500));
    assert!(input.is_long_press_active());

    input.update(false, t0 + Duration::from_millis(2600));
    assert!(!input.is_long_press_active());
}

#[test]
fn can_press_again_after_a_long_press_clear() {
    let mut input = InputHandler::new();
    let t0 = Instant::now();

    input.update(true, t0);
    assert_eq!(
        input.update(true, t0 + Duration::from_millis(2001)),
        InputEvent::LongPressClear
    );
    input.update(false, t0 + Duration::from_millis(2100));

    let t1 = t0 + Duration::from_secs(3);
    input.update(true, t1);
    assert_eq!(
        input.update(false, t1 + Duration::from_millis(100)),
        InputEvent::ShortPress
    );
}
