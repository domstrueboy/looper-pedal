use std::sync::{Arc, Mutex};

use looper_hal::{ErrorSink, HalError};

/// Where a failing stream leaves word for the UI thread.
///
/// Deliberately not part of `SharedControl`. That is atomics only and
/// sits on the path every buffer takes, so a lock there would be a lock
/// in the audio path. This is touched at most a handful of times in a
/// stream's life - and only once one has already gone wrong - so a lock
/// costs nothing and keeps the message, which a flag could not.
///
/// Latched rather than one-shot: a dead stream stays dead, and the
/// screen has to go on saying so until the device is opened again.
#[derive(Clone, Default)]
pub struct StreamFault(Arc<Mutex<Option<String>>>);

impl StreamFault {
    /// The sink to hand the backend. Keeps the first failure: what went
    /// wrong first is the cause, and everything after it is a stream
    /// that has already stopped complaining about the same thing.
    pub fn sink(&self) -> ErrorSink {
        let slot = Arc::clone(&self.0);
        Arc::new(move |error: HalError| {
            let mut held = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if held.is_none() {
                *held = Some(error.to_string());
            }
        })
    }

    /// What went wrong, if anything has.
    pub fn message(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[cfg(test)]
#[path = "fault_tests.rs"]
mod tests;
