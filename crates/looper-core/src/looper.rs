use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use looper_hal::{AudioStream, Backends, HalError};

use crate::audio::engine;
use crate::audio::shared_control::SharedControl;
use crate::state_machine::{LoopState, LoopStateMachine};
use crate::config::{self, AppConfig};
use crate::input::{InputEvent, InputHandler};
use crate::loop_mirror::{self, LoopMirror};
use crate::preroll::Preroll;

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
    /// The countdown before the first recording starts.
    preroll: Preroll,
    /// Kept here as well as in the layer stack, which lives on the audio
    /// thread out of reach - the UI needs it to show "n/max" and to know
    /// when overdubbing is still possible.
    max_layers: usize,
    /// The loop as the UI thread sees it, which is the only copy that can
    /// be written to disk - see `LoopMirror`.
    mirror: LoopMirror,
    sample_rate: u32,
    /// Where the loop is written. Held rather than looked up each time
    /// so a test can point it somewhere harmless - `config::loop_dir()`
    /// is the real per-user one.
    loop_dir: PathBuf,
    /// When the current frame was ticked. The readouts below answer from
    /// this rather than asking the clock again, so what a frame shows
    /// can't disagree with the state it was drawn beside.
    now: Instant,
    /// Holding it is what keeps the audio running; dropping it stops.
    _stream: Box<dyn AudioStream>,
}

impl LooperState {
    /// Opens the device and starts the streams, or returns why it couldn't:
    /// device/rate mismatches are user-recoverable, not bugs.
    pub fn start(backends: &Backends, settings: &AppConfig) -> Result<Self, String> {
        Self::start_in(backends, settings, config::loop_dir())
    }

    /// The same, with the saved loop somewhere other than the per-user
    /// directory. Split out for the tests, which must not read or write
    /// the loop of whoever runs them.
    pub fn start_in(
        backends: &Backends,
        settings: &AppConfig,
        loop_dir: PathBuf,
    ) -> Result<Self, String> {
        let control = Arc::new(SharedControl::new(settings.volume_pct));
        // Whatever was left from last time, if it still fits what's
        // configured now.
        let restored = loop_mirror::load(&loop_dir, settings);
        let streams = engine::build_looper_streams(
            backends,
            Arc::clone(&control),
            settings,
            &restored,
            report_stream_error(),
        )?;

        // A restored loop is there, but silent until it's asked for.
        let state_machine = if restored.is_empty() {
            LoopStateMachine::new()
        } else {
            LoopStateMachine::stopped()
        };
        control.publish_state(state_machine.state());

        Ok(Self {
            control,
            state_machine,
            input_handler: InputHandler::new(Duration::from_millis(u64::from(
                settings.long_press_ms,
            ))),
            button_held: false,
            preroll: Preroll::new(Duration::from_millis(u64::from(settings.preroll_ms))),
            max_layers: settings.max_layers as usize,
            mirror: LoopMirror::new(streams.captured, streams.sample_rate, restored),
            sample_rate: streams.sample_rate,
            loop_dir,
            now: Instant::now(),
            _stream: streams.stream,
        })
    }

    pub fn state(&self) -> LoopState {
        self.state_machine.state()
    }

    pub fn is_long_press_active(&self) -> bool {
        self.input_handler.is_long_press_active()
    }

    /// How long the control has to be held to clear, for the hint line.
    pub fn long_press_secs(&self) -> f32 {
        self.input_handler.long_press_threshold().as_secs_f32()
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
            self.mirror.remove_last_layer();
            self.save_loop();
        }
    }

    pub fn set_button_held(&mut self, held: bool) {
        self.button_held = held;
    }

    /// Seconds left of the pre-roll countdown, or 0.0 when one isn't
    /// running.
    pub fn preroll_remaining_secs(&self) -> f32 {
        self.preroll.remaining_secs(self.now)
    }

    /// How far through the pre-roll the countdown has got, 0.0-1.0.
    pub fn preroll_progress(&self) -> f32 {
        self.preroll.progress(self.now)
    }

    /// Call once per frame. Folds the spacebar and the button's latch into
    /// one `InputHandler`, so the two can't desync.
    pub fn tick(&mut self, key_held: bool, now: Instant) {
        self.now = now;
        self.log_underruns();

        let event = self.input_handler.update(key_held || self.button_held, now);
        self.apply(event, now);

        // Deliberately after the input: if a press meant to call the
        // pre-roll off lands on the same frame the countdown runs out,
        // the press should win.
        if self.preroll.take_if_elapsed(now) {
            self.state_machine.finish_arming();
            self.control.publish_state(self.state_machine.state());
        }

        if self.control.take_capture_lost() > 0 {
            self.mirror.note_lost_samples();
        }
        if self.mirror.tick(
            self.state_machine.state(),
            self.control.loop_len(),
            self.control.take_start(),
        ) {
            self.save_loop();
        }
    }

    /// Writes the loop out, so closing the app doesn't lose it. A failure
    /// is reported and otherwise ignored: it isn't worth interrupting
    /// playing over.
    fn save_loop(&self) {
        if let Err(err) = self.mirror.save(&self.loop_dir) {
            eprintln!("could not save the loop: {err}");
        }
    }

    /// Publishes the resulting state in the same step as changing it, so
    /// that pairing can't be forgotten at a call site.
    fn apply(&mut self, event: InputEvent, now: Instant) {
        match event {
            InputEvent::ShortPress => {
                // A pre-roll is only for the first recording - once a
                // loop is playing you're already in time with it.
                if self.state() == LoopState::Idle && self.preroll.is_enabled() {
                    self.state_machine.arm();
                    self.preroll.start(now);
                } else {
                    self.state_machine.press();
                    self.preroll.cancel();
                }
                self.control.publish_state(self.state_machine.state());
            }
            InputEvent::LongPressClear => self.clear(),
            InputEvent::None => {}
        }
    }

    fn clear(&mut self) {
        self.state_machine.clear();
        self.preroll.cancel();
        self.control.publish_state(self.state_machine.state());
        self.control.request_clear();
        self.mirror.clear();
        loop_mirror::delete(&self.loop_dir);
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

/// Where a running stream reports trouble.
///
/// Still only stderr, which a release build has no console for - the
/// in-app surface is its own task. What has changed is that there is now
/// one place to put it, rather than a callback buried in the backend.
fn report_stream_error() -> looper_hal::ErrorSink {
    Arc::new(|error: HalError| eprintln!("{error}"))
}

#[cfg(test)]
#[path = "looper_tests.rs"]
mod tests;
