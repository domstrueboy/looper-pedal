use std::sync::Arc;
use std::time::Instant;

use crate::audio::engine;
use crate::audio::shared_control::SharedControl;
use crate::audio::state_machine::{LoopState, LoopStateMachine};
use crate::input::{InputEvent, InputHandler};

/// State behind the looper screen, with no `egui::` types in it: the
/// state machine (owned here, on the UI thread), the lock-free relay into
/// the audio thread, and the live streams whose callbacks do the actual
/// audio work.
pub struct LooperState {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
    // Latches true the moment the button is pressed and stays true until
    // the pointer button is released globally, regardless of whether the
    // pointer drifts off the button's rect mid-hold - egui's own hover
    // tracking would otherwise read as a release the instant the cursor
    // leaves the widget, even with the mouse button still down, which is
    // exactly what broke long-press-clear from the on-screen button
    // (spacebar has no such issue - key_down has no positional
    // component). Kept here rather than in the renderer because it has to
    // survive across frames; the renderer is what updates it.
    button_held: bool,
    sample_rate: u32,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl LooperState {
    /// Opens `device_name` and starts the audio streams, or returns why it
    /// couldn't - device/rate mismatches are user-recoverable, not
    /// programming errors.
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

    /// Recorded loop length in seconds (0.0 if empty) and playback
    /// position through it as a 0.0-1.0 fraction.
    pub fn loop_duration_and_progress(&self) -> (f32, f32) {
        self.control.loop_duration_and_progress(self.sample_rate)
    }

    /// Whether the on-screen button is currently being held - see the
    /// `button_held` field comment for why the renderer latches this
    /// rather than asking egui each frame.
    pub fn set_button_held(&mut self, held: bool) {
        self.button_held = held;
    }

    /// Call once per frame. `key_held` is the spacebar; the on-screen
    /// button's latch is folded in here so both drive the exact same
    /// `InputHandler` and can never desync.
    pub fn tick(&mut self, key_held: bool, now: Instant) {
        self.log_underruns();
        let event = self.input_handler.update(key_held || self.button_held, now);
        self.apply(event);
    }

    /// Applies an input event, always publishing the resulting state to
    /// the audio thread in the same step - keeps that pairing from being
    /// duplicated (and potentially forgotten) at each call site.
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

    /// Underrun counts are accumulated lock-free on the audio threads and
    /// drained here instead of being logged from inside the callbacks
    /// themselves (stdio isn't real-time safe). Note this output has
    /// nowhere to go once release builds lose their console - see PLAN.md's
    /// "Release-build diagnostics" open decision.
    fn log_underruns(&self) {
        let (input_underruns, output_underruns) = self.control.take_underrun_counts();
        if input_underruns > 0 {
            eprintln!(
                "input stream fell behind {input_underruns} time(s): try increasing latency"
            );
        }
        if output_underruns > 0 {
            eprintln!(
                "output stream fell behind {output_underruns} time(s): try increasing latency"
            );
        }
    }
}
