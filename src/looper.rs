use std::sync::Arc;
use std::time::Instant;

use crate::audio::engine;
use crate::audio::shared_control::SharedControl;
use crate::audio::state_machine::{LoopState, LoopStateMachine};
use crate::input::{InputEvent, InputHandler};

/// State behind the looper screen: the state machine (owned here, on the
/// UI thread), the relay into the audio thread, and the live streams.
pub struct LooperState {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
    // Latched on press, cleared only when the pointer is released
    // anywhere: egui reads a cursor drifting off the button as a release
    // even while it's still held, which broke long-press-clear from the
    // on-screen button. Set by the renderer, kept here to survive frames.
    button_held: bool,
    sample_rate: u32,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl LooperState {
    /// Opens the device and starts the streams, or returns why it couldn't:
    /// device/rate mismatches are user-recoverable, not bugs.
    pub fn start(
        device_name: &str,
        sample_rate: u32,
        input_channel: u16,
        volume_pct: u32,
    ) -> Result<Self, String> {
        let control = Arc::new(SharedControl::new(volume_pct));
        let (_input_stream, _output_stream, sample_rate) = engine::build_looper_streams(
            Arc::clone(&control),
            device_name,
            sample_rate,
            input_channel,
        )?;
        Ok(Self {
            control,
            state_machine: LoopStateMachine::new(),
            input_handler: InputHandler::new(),
            button_held: false,
            sample_rate,
            _input_stream,
            _output_stream,
        })
    }

    pub fn state(&self) -> LoopState {
        self.state_machine.state()
    }

    pub fn is_long_press_active(&self) -> bool {
        self.input_handler.is_long_press_active()
    }

    /// Loop length in seconds (0.0 if empty) and playback position as a
    /// 0.0-1.0 fraction.
    pub fn loop_duration_and_progress(&self) -> (f32, f32) {
        self.control.loop_duration_and_progress(self.sample_rate)
    }

    pub fn set_button_held(&mut self, held: bool) {
        self.button_held = held;
    }

    /// Call once per frame. Folds the spacebar and the button's latch into
    /// one `InputHandler`, so the two can't desync.
    pub fn tick(&mut self, key_held: bool, now: Instant) {
        self.log_underruns();
        let event = self.input_handler.update(key_held || self.button_held, now);
        self.apply(event);
    }

    /// Publishes the resulting state in the same step as changing it, so
    /// that pairing can't be forgotten at a call site.
    fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::ShortPress => {
                self.state_machine.press();
                self.control.publish_state(self.state_machine.state());
            }
            InputEvent::LongPressClear => {
                self.state_machine.clear();
                self.control.publish_state(self.state_machine.state());
                self.control.request_clear();
            }
            InputEvent::None => {}
        }
    }

    /// Drained and logged here rather than in the callbacks, since stdio
    /// isn't real-time safe.
    fn log_underruns(&self) {
        let (input_underruns, output_underruns) = self.control.take_underrun_counts();
        if input_underruns > 0 {
            eprintln!("input stream fell behind {input_underruns} time(s): try increasing latency");
        }
        if output_underruns > 0 {
            eprintln!(
                "output stream fell behind {output_underruns} time(s): try increasing latency"
            );
        }
    }
}
