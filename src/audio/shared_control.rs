use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use super::state_machine::LoopState;

/// Lock-free relay of state changes into the audio callback. The
/// `LoopStateMachine` itself stays single-owner on the UI thread (see
/// `main.rs`) - only the resulting state value and a one-shot clear flag
/// cross the thread boundary, both via atomics, never a lock.
pub struct SharedControl {
    state: AtomicU8,
    clear_requested: AtomicBool,
    // Telemetry flowing the other way (audio thread -> UI thread), for the
    // GUI's loop-duration display and playback-position progress bar.
    loop_len: AtomicUsize,
    play_pos: AtomicUsize,
    // Underrun counters, also audio -> UI. Plain monotonic counts with no
    // dependent data, so Relaxed is enough - unlike `state`/`play_pos`
    // there's nothing else that needs to be ordered around them. Logging
    // underruns is done from the UI thread instead of inside the audio
    // callback itself, since stdio there would add I/O to a real-time
    // callback exactly when it's already falling behind.
    input_underruns: AtomicUsize,
    output_underruns: AtomicUsize,
}

impl SharedControl {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(LoopState::Idle as u8),
            clear_requested: AtomicBool::new(false),
            loop_len: AtomicUsize::new(0),
            play_pos: AtomicUsize::new(0),
            input_underruns: AtomicUsize::new(0),
            output_underruns: AtomicUsize::new(0),
        }
    }

    pub fn publish_state(&self, state: LoopState) {
        self.state.store(state as u8, Ordering::Release);
    }

    pub fn request_clear(&self) {
        self.clear_requested.store(true, Ordering::Release);
    }

    /// Recorded loop duration in seconds (0.0 if empty) and current
    /// playback position as a 0.0-1.0 fraction through the loop. The loop
    /// buffer is mono (one sample per frame), so just `sample_rate` is
    /// needed - no channel count.
    pub fn loop_duration_and_progress(&self, sample_rate: u32) -> (f32, f32) {
        let loop_len = self.loop_len.load(Ordering::Acquire);
        let play_pos = self.play_pos.load(Ordering::Acquire);
        let samples_per_second = (sample_rate as usize).max(1);

        let duration_secs = loop_len as f32 / samples_per_second as f32;
        let progress_fraction = if loop_len > 0 {
            play_pos as f32 / loop_len as f32
        } else {
            0.0
        };
        (duration_secs, progress_fraction)
    }

    pub(super) fn load_state(&self) -> LoopState {
        match self.state.load(Ordering::Acquire) {
            0 => LoopState::Idle,
            1 => LoopState::Recording,
            2 => LoopState::Looping,
            _ => LoopState::Stopped,
        }
    }

    pub(super) fn take_clear_request(&self) -> bool {
        self.clear_requested.swap(false, Ordering::AcqRel)
    }

    pub(super) fn publish_loop_progress(&self, len: usize, pos: usize) {
        self.loop_len.store(len, Ordering::Release);
        self.play_pos.store(pos, Ordering::Release);
    }

    pub(super) fn note_input_underrun(&self) {
        self.input_underruns.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_output_underrun(&self) {
        self.output_underruns.fetch_add(1, Ordering::Relaxed);
    }

    /// Drains the underrun counts accumulated since the last call, for the
    /// UI thread to log. `(input, output)` - see the fields above for what
    /// each side means.
    pub fn take_underrun_counts(&self) -> (usize, usize) {
        (
            self.input_underruns.swap(0, Ordering::Relaxed),
            self.output_underruns.swap(0, Ordering::Relaxed),
        )
    }
}
