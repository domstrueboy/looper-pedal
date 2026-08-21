use super::*;

#[test]
fn starts_idle() {
    let sm = LoopStateMachine::new();
    assert_eq!(sm.state(), LoopState::Idle);
}

#[test]
fn press_cycles_through_full_sequence() {
    let mut sm = LoopStateMachine::new();

    sm.press();
    assert_eq!(sm.state(), LoopState::Recording);

    sm.press();
    assert_eq!(sm.state(), LoopState::Looping);

    sm.press();
    assert_eq!(sm.state(), LoopState::Stopped);

    sm.press();
    assert_eq!(sm.state(), LoopState::Looping);

    sm.press();
    assert_eq!(sm.state(), LoopState::Stopped);
}

#[test]
fn clear_from_idle_stays_idle() {
    let mut sm = LoopStateMachine::new();
    sm.clear();
    assert_eq!(sm.state(), LoopState::Idle);
}

#[test]
fn clear_from_recording_returns_to_idle() {
    let mut sm = LoopStateMachine::new();
    sm.press();
    assert_eq!(sm.state(), LoopState::Recording);
    sm.clear();
    assert_eq!(sm.state(), LoopState::Idle);
}

#[test]
fn clear_from_looping_returns_to_idle() {
    let mut sm = LoopStateMachine::new();
    sm.press();
    sm.press();
    assert_eq!(sm.state(), LoopState::Looping);
    sm.clear();
    assert_eq!(sm.state(), LoopState::Idle);
}

#[test]
fn clear_from_stopped_returns_to_idle() {
    let mut sm = LoopStateMachine::new();
    sm.press();
    sm.press();
    sm.press();
    assert_eq!(sm.state(), LoopState::Stopped);
    sm.clear();
    assert_eq!(sm.state(), LoopState::Idle);
}

#[test]
fn can_record_again_after_clear() {
    let mut sm = LoopStateMachine::new();
    sm.press();
    sm.press();
    sm.clear();
    sm.press();
    assert_eq!(sm.state(), LoopState::Recording);
}
