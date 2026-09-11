use super::*;

use crate::mock::{self, mock};

#[test]
fn a_registry_finds_a_backend_by_the_name_a_config_stored() {
    let (backend, _driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);

    assert!(backends.by_name("mock").is_some());
    assert!(backends.by_name("asio").is_none(), "not in this build");
}

#[test]
fn an_empty_registry_is_empty() {
    let backends = Backends::new(Vec::new());

    assert!(backends.is_empty());
    assert!(backends.devices().is_empty());
    assert!(backends.get(BackendId::MOCK).is_none());
}

#[test]
fn devices_are_gathered_across_backends() {
    let (first, _a) = mock();
    let (second, _b) = mock();
    let backends = Backends::new(vec![Box::new(first), Box::new(second)]);

    assert_eq!(backends.devices().len(), 2);
}

/// A backend whose driver isn't installed, which is the normal state of
/// ASIO on a machine that has never had an interface plugged in.
struct Absent;

impl AudioBackend for Absent {
    fn id(&self) -> BackendId {
        BackendId::ASIO
    }
    fn label(&self) -> &str {
        "ASIO"
    }
    fn devices(&self) -> HalResult<Vec<DeviceInfo>> {
        Err(HalError::BackendUnavailable {
            backend: BackendId::ASIO,
            detail: "no driver installed".to_string(),
        })
    }
    fn caps(&self, _device: &DeviceId) -> HalResult<DeviceCaps> {
        unreachable!("nothing to ask about")
    }
    fn open(&self, _request: &StreamRequest) -> HalResult<Box<dyn OpenDevice>> {
        unreachable!("nothing to open")
    }
}

#[test]
fn a_backend_that_cannot_enumerate_does_not_hide_the_others() {
    // Otherwise having no ASIO driver would present as having no audio
    // devices at all, rather than as having the WASAPI ones.
    let (working, _driver) = mock();
    let backends = Backends::new(vec![Box::new(Absent), Box::new(working)]);

    let devices = backends.devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id.backend, BackendId::MOCK);
}

#[test]
fn an_input_and_output_from_different_backends_are_refused() {
    // They would be driven by separate clocks with no way to agree on a
    // buffer size, so this is caught before anything opens.
    let (backend, _driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);

    let crossed = StreamRequest {
        input: DeviceId::new(BackendId::MOCK, mock::DEVICE),
        output: DeviceId::new(BackendId::WASAPI, "Speakers"),
        sample_rate: 48_000,
    };

    assert!(matches!(
        backends.open(&crossed),
        Err(HalError::CrossBackend { .. })
    ));
}

#[test]
fn opening_through_a_backend_that_is_not_there_says_so() {
    let backends = Backends::new(Vec::new());

    assert!(matches!(
        backends.open(&mock::request(48_000)),
        Err(HalError::BackendUnavailable { .. })
    ));
}

#[test]
fn the_registry_opens_through_the_right_backend() {
    let (backend, driver) = mock();
    let backends = Backends::new(vec![Box::new(backend)]);

    let open = backends.open(&mock::request(96_000)).expect("opens");

    assert_eq!(open.format().sample_rate, 96_000);
    assert!(!driver.is_running(), "still only negotiated");
}

#[test]
fn errors_read_as_something_worth_showing_a_person() {
    // These reach the settings screen verbatim, so they have to make
    // sense to whoever is trying to get their interface working.
    assert_eq!(
        HalError::DeviceNotFound {
            backend: BackendId::ASIO,
            device: "Audient".to_string(),
        }
        .to_string(),
        "asio device 'Audient' not found"
    );
    assert_eq!(
        HalError::CrossBackend {
            input: BackendId::ASIO,
            output: BackendId::WASAPI,
        }
        .to_string(),
        "input is asio and output is wasapi; they must be the same"
    );
}
