//! A backend with no device behind it, driven by whoever is testing.
//!
//! Always compiled, and costing no dependencies, because it is the only
//! way to drive the whole path - processors, start, stop - without an
//! audio interface plugged in. Opening a real device in a test needs
//! hardware, a driver, and an exclusive claim on it that a second test
//! would fight over.

use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    AudioBackend, AudioStream, BackendId, DeviceCaps, DeviceId, DeviceInfo, Direction, ErrorSink,
    HalError, HalResult, InputProcessor, MAX_BLOCK_FRAMES, OpenDevice, OutputProcessor,
    StreamFormat, StreamRequest, CANDIDATE_SAMPLE_RATES,
};

/// The one device this backend reports.
pub const DEVICE: &str = "Mock Device";


struct Running {
    input: Box<dyn InputProcessor>,
    output: Box<dyn OutputProcessor>,
}

#[derive(Default)]
struct Shared {
    running: Option<Running>,
    /// Counts every `start` so a test can tell "never started" from
    /// "started and then stopped".
    starts: usize,
}

/// A mock backend, and the handle that pretends to be its driver. Both
/// refer to the same stream, so starting one is visible from the other.
pub fn mock() -> (MockBackend, MockDriver) {
    mock_with(1, 1)
}

/// The same, with the channel counts a test wants. The device reports
/// them as one duplex device, the way ASIO does.
pub fn mock_with(input_channels: u16, output_channels: u16) -> (MockBackend, MockDriver) {
    let shared = Rc::new(RefCell::new(Shared::default()));
    let backend = MockBackend {
        shared: Rc::clone(&shared),
        input_channels,
        output_channels,
        granted_rate: None,
    };
    let driver = MockDriver {
        shared,
        input_channels,
        output_channels,
    };
    (backend, driver)
}

pub struct MockBackend {
    shared: Rc<RefCell<Shared>>,
    input_channels: u16,
    output_channels: u16,
    granted_rate: Option<u32>,
}

impl MockBackend {
    /// Grant this rate whatever was asked for, the way a shared-mode
    /// endpoint hands back its own mix rate. For proving that a caller
    /// reads the granted format instead of assuming its request stood.
    pub fn always_grants(mut self, sample_rate: u32) -> Self {
        self.granted_rate = Some(sample_rate);
        self
    }

    fn device_id() -> DeviceId {
        DeviceId::new(BackendId::MOCK, DEVICE)
    }
}

impl AudioBackend for MockBackend {
    fn id(&self) -> BackendId {
        BackendId::MOCK
    }

    fn label(&self) -> &str {
        "Mock"
    }

    fn devices(&self) -> HalResult<Vec<DeviceInfo>> {
        Ok(vec![DeviceInfo {
            id: Self::device_id(),
            direction: Direction::Duplex,
        }])
    }

    fn caps(&self, device: &DeviceId) -> HalResult<DeviceCaps> {
        if device.name != DEVICE {
            return Err(HalError::DeviceNotFound {
                backend: BackendId::MOCK,
                device: device.name.clone(),
            });
        }
        Ok(DeviceCaps {
            sample_rates: CANDIDATE_SAMPLE_RATES.to_vec(),
            input_channels: self.input_channels,
            output_channels: self.output_channels,
        })
    }

    fn open(&self, request: &StreamRequest) -> HalResult<Box<dyn OpenDevice>> {
        for device in [&request.input, &request.output] {
            if device.name != DEVICE {
                return Err(HalError::DeviceNotFound {
                    backend: BackendId::MOCK,
                    device: device.name.clone(),
                });
            }
        }
        let sample_rate = match self.granted_rate {
            Some(rate) => rate,
            None if CANDIDATE_SAMPLE_RATES.contains(&request.sample_rate) => request.sample_rate,
            None => {
                return Err(HalError::UnsupportedConfig(format!(
                    "{} Hz is not supported by '{DEVICE}'",
                    request.sample_rate
                )));
            }
        };

        Ok(Box::new(MockOpenDevice {
            shared: Rc::clone(&self.shared),
            format: StreamFormat {
                sample_rate,
                input_channels: self.input_channels,
                output_channels: self.output_channels,
                max_block_frames: MAX_BLOCK_FRAMES,
            },
        }))
    }
}

struct MockOpenDevice {
    shared: Rc<RefCell<Shared>>,
    format: StreamFormat,
}

impl OpenDevice for MockOpenDevice {
    fn format(&self) -> StreamFormat {
        self.format
    }

    fn start(
        self: Box<Self>,
        input: Box<dyn InputProcessor>,
        output: Box<dyn OutputProcessor>,
        _on_error: ErrorSink,
    ) -> HalResult<Box<dyn AudioStream>> {
        let mut shared = self.shared.borrow_mut();
        shared.running = Some(Running { input, output });
        shared.starts += 1;
        drop(shared);

        Ok(Box::new(MockStream {
            shared: self.shared,
            format: self.format,
        }))
    }
}

struct MockStream {
    shared: Rc<RefCell<Shared>>,
    format: StreamFormat,
}

impl AudioStream for MockStream {
    fn format(&self) -> StreamFormat {
        self.format
    }
}

/// Dropping the handle stops the audio, which is the contract every
/// backend keeps - so the mock has to keep it too, or a test would pass
/// where a real device leaves a stream running.
impl Drop for MockStream {
    fn drop(&mut self) {
        self.shared.borrow_mut().running = None;
    }
}

/// The driver side: what a test calls to make callbacks happen.
pub struct MockDriver {
    shared: Rc<RefCell<Shared>>,
    input_channels: u16,
    output_channels: u16,
}

impl MockDriver {
    /// One buffer switch: the capture callback, then the render one, in
    /// the order a driver runs them. `input` is interleaved; so is what
    /// comes back.
    ///
    /// Panics if nothing is running, because a test that meant to drive
    /// a stream and silently drove nothing would pass for the wrong
    /// reason.
    pub fn cycle(&self, input: &[f32]) -> Vec<f32> {
        let frames = input.len() / self.input_channels as usize;
        let mut output = vec![0.0; frames * self.output_channels as usize];

        let mut shared = self.shared.borrow_mut();
        let running = shared
            .running
            .as_mut()
            .expect("cycle() with no stream running");

        // Chunked the way a real backend chunks, so a processor that
        // relies on never seeing more than `max_block_frames` is held to
        // the same bound here as on a device.
        let in_block = MAX_BLOCK_FRAMES * self.input_channels as usize;
        let out_block = MAX_BLOCK_FRAMES * self.output_channels as usize;
        for (captured, rendered) in input.chunks(in_block).zip(output.chunks_mut(out_block)) {
            running.input.process(captured);
            running.output.process(rendered);
        }
        output
    }

    /// Whether a stream is open right now.
    pub fn is_running(&self) -> bool {
        self.shared.borrow().running.is_some()
    }

    /// How many times a stream has been started, so "never opened" can
    /// be told from "opened and closed again".
    pub fn starts(&self) -> usize {
        self.shared.borrow().starts
    }

    /// A sink that files errors here for a test to read back.
    pub fn error_sink(&self) -> (ErrorSink, ErrorLog) {
        let log = ErrorLog::default();
        let sink = log.clone();
        (
            std::sync::Arc::new(move |error| sink.0.lock().expect("not poisoned").push(error)),
            log,
        )
    }

}

/// Errors a stream reported, for a test to assert on. Behind a mutex
/// rather than a `RefCell` because `ErrorSink` has to be `Send + Sync`.
#[derive(Clone, Default)]
pub struct ErrorLog(std::sync::Arc<std::sync::Mutex<Vec<HalError>>>);

impl ErrorLog {
    pub fn taken(&self) -> Vec<HalError> {
        std::mem::take(&mut *self.0.lock().expect("not poisoned"))
    }
}

/// A request naming the mock device both ways, which is what a duplex
/// backend gets handed.
pub fn request(sample_rate: u32) -> StreamRequest {
    StreamRequest {
        input: DeviceId::new(BackendId::MOCK, DEVICE),
        output: DeviceId::new(BackendId::MOCK, DEVICE),
        sample_rate,
    }
}

#[cfg(test)]
#[path = "mock_tests.rs"]
mod tests;
