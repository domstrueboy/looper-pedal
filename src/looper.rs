use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::engine;
use crate::audio::shared_control::SharedControl;
use crate::audio::state_machine::{LoopState, LoopStateMachine};
use crate::config::AppConfig;
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
    /// How long to count down before the first recording starts; zero
    /// means start immediately, which is what it was before this
    /// existed.
    preroll: Duration,
    /// Set while a pre-roll is counting down, cleared the moment it
    /// elapses or is called off.
    armed_at: Option<Instant>,
    /// Kept here as well as in the layer stack, which lives on the audio
    /// thread out of reach - the UI needs it to show "n/max" and to know
    /// when overdubbing is still possible.
    max_layers: usize,
    sample_rate: u32,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl LooperState {
    /// Opens the device and starts the streams, or returns why it couldn't:
    /// device/rate mismatches are user-recoverable, not bugs.
    pub fn start(config: &AppConfig) -> Result<Self, String> {
        let control = Arc::new(SharedControl::new(config.volume_pct));
        let (_input_stream, _output_stream, sample_rate) =
            engine::build_looper_streams(Arc::clone(&control), config)?;
        Ok(Self {
            control,
            state_machine: LoopStateMachine::new(),
            input_handler: InputHandler::new(Duration::from_millis(u64::from(
                config.long_press_ms,
            ))),
            button_held: false,
            preroll: Duration::from_millis(u64::from(config.preroll_ms)),
            armed_at: None,
            max_layers: config.max_layers as usize,
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

    pub fn layer_count(&self) -> usize {
        self.control.layer_count()
    }

    pub fn max_layers(&self) -> usize {
        self.max_layers
    }

    /// Whether the overdub control would do anything: there has to be a
    /// loop playing, and a layer left to record into.
    pub fn can_overdub(&self) -> bool {
        match self.state() {
            LoopState::Looping => self.layer_count() < self.max_layers,
            LoopState::Overdubbing => true,
            _ => false,
        }
    }

    /// The overdub control: opens a layer over the playing loop, or
    /// closes the one in progress.
    pub fn toggle_overdub(&mut self) {
        if !self.can_overdub() {
            return;
        }
        self.state_machine.toggle_overdub();
        self.control.publish_state(self.state_machine.state());
    }

    /// Only while nothing is being recorded - dropping a finished layer
    /// mid-take would be ambiguous about which one is meant.
    pub fn can_remove_layer(&self) -> bool {
        self.layer_count() > 0 && matches!(self.state(), LoopState::Looping | LoopState::Stopped)
    }

    /// Drops the newest layer. Dropping the only one leaves nothing to
    /// play, so that case is a clear and takes the state back to Idle
    /// with it.
    pub fn remove_last_layer(&mut self) {
        if !self.can_remove_layer() {
            return;
        }
        if self.layer_count() <= 1 {
            self.clear();
        } else {
            self.control.request_remove_layer();
        }
    }

    pub fn set_button_held(&mut self, held: bool) {
        self.button_held = held;
    }

    /// Seconds left of the pre-roll countdown, or 0.0 when one isn't
    /// running.
    pub fn preroll_remaining_secs(&self) -> f32 {
        match self.armed_at {
            Some(armed_at) => {
                (self.preroll.as_secs_f32() - armed_at.elapsed().as_secs_f32()).max(0.0)
            }
            None => 0.0,
        }
    }

    /// How far through the pre-roll the countdown has got, 0.0-1.0.
    pub fn preroll_progress(&self) -> f32 {
        match self.armed_at {
            Some(armed_at) if !self.preroll.is_zero() => {
                (armed_at.elapsed().as_secs_f32() / self.preroll.as_secs_f32()).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// Call once per frame. Folds the spacebar and the button's latch into
    /// one `InputHandler`, so the two can't desync.
    pub fn tick(&mut self, key_held: bool, now: Instant) {
        self.log_underruns();

        let event = self.input_handler.update(key_held || self.button_held, now);
        self.apply(event, now);

        // Deliberately after the input: if a press meant to call the
        // pre-roll off lands on the same frame the countdown runs out,
        // the press should win.
        if let Some(armed_at) = self.armed_at {
            if now.duration_since(armed_at) >= self.preroll {
                self.state_machine.finish_arming();
                self.armed_at = None;
                self.control.publish_state(self.state_machine.state());
            }
        }
    }

    /// Publishes the resulting state in the same step as changing it, so
    /// that pairing can't be forgotten at a call site.
    fn apply(&mut self, event: InputEvent, now: Instant) {
        match event {
            InputEvent::ShortPress => {
                // A pre-roll is only for the first recording - once a
                // loop is playing you're already in time with it.
                if self.state() == LoopState::Idle && !self.preroll.is_zero() {
                    self.state_machine.arm();
                    self.armed_at = Some(now);
                } else {
                    self.state_machine.press();
                    self.armed_at = None;
                }
                self.control.publish_state(self.state_machine.state());
            }
            InputEvent::LongPressClear => self.clear(),
            InputEvent::None => {}
        }
    }

    fn clear(&mut self) {
        self.state_machine.clear();
        self.armed_at = None;
        self.control.publish_state(self.state_machine.state());
        self.control.request_clear();
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
