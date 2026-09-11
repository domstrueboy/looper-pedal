use super::*;

#[test]
fn nothing_is_wrong_until_something_goes_wrong() {
    assert_eq!(StreamFault::default().message(), None);
}

#[test]
fn a_failure_reaches_the_ui_side_with_its_message() {
    let fault = StreamFault::default();
    fault.sink()(HalError::Runtime("the device was unplugged".to_string()));

    let message = fault.message().expect("something was recorded");
    assert!(message.contains("unplugged"), "got: {message}");
}

#[test]
fn the_first_failure_is_the_one_kept() {
    // A stream that has stopped tends to say so repeatedly. What went
    // wrong first is the cause; the rest are echoes of it.
    let fault = StreamFault::default();
    let sink = fault.sink();
    sink(HalError::Runtime("the device was unplugged".to_string()));
    sink(HalError::Runtime("a later complaint".to_string()));

    assert!(fault.message().expect("recorded").contains("unplugged"));
}

#[test]
fn it_latches_rather_than_clearing_when_read() {
    // A dead stream stays dead, so the screen has to go on saying so.
    let fault = StreamFault::default();
    fault.sink()(HalError::Runtime("gone".to_string()));

    assert!(fault.message().is_some());
    assert!(fault.message().is_some(), "reading it does not clear it");
}
