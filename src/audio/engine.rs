use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Producer, Split},
};

use super::loop_stack::LoopStack;
use super::shared_control::SharedControl;
use crate::config::AppConfig;
use crate::state_machine::LoopState;

/// Bounds one callback's worth of work at a fixed size, so the scratch
/// buffers can be taken up front. Not a setting - it's about what the
/// driver might hand us, not about how anyone wants the app to behave.
const SCRATCH_CAPACITY: usize = 32768;

// Offered in settings, filtered to what the chosen device supports.
const CANDIDATE_SAMPLE_RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

/// Reports rather than panics: without a console there'd be nothing to
/// see if this failed, so the settings screen shows it instead.
fn asio_host() -> Result<cpal::Host, String> {
    cpal::host_from_id(cpal::HostId::Asio).map_err(|e| format!("ASIO host unavailable: {e}"))
}

pub fn available_asio_devices() -> Result<Vec<String>, String> {
    asio_host()?
        .devices()
        .map_err(|e| format!("failed to enumerate ASIO devices: {e}"))
        .map(|devices| devices.map(|d| d.to_string()).collect())
}

fn find_device(device_name: &str) -> Result<cpal::Device, String> {
    asio_host()?
        .devices()
        .map_err(|e| format!("failed to enumerate ASIO devices: {e}"))?
        .find(|d| d.to_string() == device_name)
        .ok_or_else(|| format!("ASIO device '{device_name}' not found"))
}

/// Candidate rates `device_name` actually supports, in i32.
fn supported_sample_rates(device_name: &str) -> Result<Vec<u32>, String> {
    let device = find_device(device_name)?;
    let configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| format!("failed to query supported configs: {e}"))?
        .collect();

    Ok(CANDIDATE_SAMPLE_RATES
        .into_iter()
        .filter(|&rate| {
            configs
                .iter()
                .any(|c| c.sample_format() == cpal::SampleFormat::I32 && c.contains_rate(rate))
        })
        .collect())
}

/// Hardware input channel count.
fn input_channel_count(device_name: &str) -> Result<u16, String> {
    let device = find_device(device_name)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get default input config: {e}"))?;
    Ok(config.channels())
}

/// Both of the above, which is the only way the settings screen wants
/// them. Falls back to empty/zero rather than surfacing the error - a
/// device that answers neither can't be started, and the screen says so
/// from the empty lists themselves.
pub fn rates_and_channels(device_name: &str) -> (Vec<u32>, u16) {
    (
        supported_sample_rates(device_name).unwrap_or_default(),
        input_channel_count(device_name).unwrap_or(0),
    )
}

/// Negotiates an input/output config at `sample_rate`, asserting the i32
/// format this project is built around (the iD4 MkII's native format).
/// Errors rather than panics: mismatches are recoverable in settings.
fn open_device_and_config(
    device_name: &str,
    sample_rate: u32,
) -> Result<(cpal::Device, cpal::StreamConfig), String> {
    let device = find_device(device_name)?;

    let input_config = device
        .default_input_config()
        .map_err(|e| format!("failed to get default input config: {e}"))?;
    let output_config = device
        .default_output_config()
        .map_err(|e| format!("failed to get default output config: {e}"))?;
    if input_config.sample_format() != output_config.sample_format() {
        return Err("input and output must share a sample format".to_string());
    }
    if input_config.sample_format() != cpal::SampleFormat::I32 {
        return Err(format!(
            "unsupported sample format {:?} (expected i32)",
            input_config.sample_format()
        ));
    }

    // `supported_output_configs()` lists a SEPARATE entry per channel count
    // at each rate, so the device's full count has to be matched explicitly
    // or we'd silently open the first (e.g. mono) entry.
    let full_channels = input_config.channels();
    let output_configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| format!("failed to query supported configs: {e}"))?
        .collect();
    let matching_range = output_configs
        .into_iter()
        .find(|c| {
            c.sample_format() == cpal::SampleFormat::I32
                && c.channels() == full_channels
                && c.contains_rate(sample_rate)
        })
        .ok_or_else(|| format!("{sample_rate} Hz is not supported by '{device_name}'"))?;

    let config: cpal::StreamConfig = matching_range.with_sample_rate(sample_rate).into();
    println!(
        "Stream config: {} Hz, {} channel(s), buffer size: {:?}",
        config.sample_rate, config.channels, config.buffer_size
    );

    Ok((device, config))
}

/// Seconds of captured audio the UI thread's copy can fall behind by
/// before samples start being dropped. Generous: losing any means the
/// recorded loop can't be saved.
const CAPTURE_BUFFER_SECONDS: usize = 2;

/// The live streams, plus the channel the UI thread reads captured
/// samples from - it keeps its own copy of the loop, since the layer
/// stack itself is out of reach inside the output callback.
pub struct LooperStreams {
    pub input: cpal::Stream,
    pub output: cpal::Stream,
    pub sample_rate: u32,
    pub captured: HeapCons<i32>,
}

/// The input half of the audio path: one interleaved buffer from the
/// driver, reduced to the chosen channel as mono and handed to everyone
/// who wants it.
///
/// A struct rather than a closure body so the path can be driven by a
/// test - a cpal callback needs an open device, which makes anything
/// written inside one unreachable.
struct InputPath {
    control: Arc<SharedControl>,
    channels: u16,
    input_channel: u16,
    scratch: Vec<i32>,
    passthrough: HeapProd<i32>,
    recorder: HeapProd<i32>,
    capture: HeapProd<i32>,
}

impl InputPath {
    fn process(&mut self, data: &[i32]) {
        // Bound each chunk to the fixed-size scratch buffer whatever the
        // driver hands us: overrunning it would panic inside a real-time
        // callback. Never happens in practice, but cheap.
        for chunk in data.chunks(SCRATCH_CAPACITY * self.channels as usize) {
            let frames = chunk.len() / self.channels as usize;
            for (i, frame) in chunk.chunks_exact(self.channels as usize).enumerate() {
                self.scratch[i] = frame[self.input_channel as usize];
            }
            let mono = &self.scratch[..frames];

            // A full ring means the far side hasn't drained it, so it is
            // the output that fell behind, not this callback.
            if self.passthrough.push_slice(mono) < mono.len() {
                self.control.note_output_underrun();
            }
            if matches!(
                self.control.load_state(),
                LoopState::Recording | LoopState::Overdubbing
            ) {
                self.recorder.push_slice(mono);
                let mirrored = self.capture.push_slice(mono);
                if mirrored < mono.len() {
                    self.control.note_capture_lost(mono.len() - mirrored);
                }
            }
        }
    }
}

/// The output half: the dry signal, the loop mixed onto it, and the
/// overdub written back. Owns `LoopStack` outright - it lives here and
/// nowhere else, which is why nothing needs a lock to reach it.
struct OutputPath {
    control: Arc<SharedControl>,
    channels: u16,
    dry: Vec<i32>,
    loop_out: Vec<i32>,
    recorded: Vec<i32>,
    passthrough: HeapCons<i32>,
    recorder: HeapCons<i32>,
    stack: LoopStack,
    /// The callback only ever sees the published state, never the
    /// transitions, so layer bookkeeping keys off this changing - see
    /// `apply_state_change`.
    previous_state: LoopState,
}

impl OutputPath {
    fn process(&mut self, data: &mut [i32]) {
        // Same chunk-bounding as the input path.
        for out in data.chunks_mut(SCRATCH_CAPACITY * self.channels as usize) {
            let frames = out.len() / self.channels as usize;
            let dry = &mut self.dry[..frames];

            // A short read means the input hasn't filled the ring yet,
            // so it is the input that fell behind.
            let read = self.passthrough.pop_slice(dry);
            if read < frames {
                dry[read..].fill(0);
                self.control.note_input_underrun();
            }

            let state = self.control.load_state();
            if state != self.previous_state {
                apply_state_change(&mut self.stack, &self.control, self.previous_state, state);
                self.previous_state = state;
            }

            if self.control.take_clear_request() {
                self.stack.clear();
            }
            if self.control.take_remove_layer_request() {
                self.stack.remove_last_layer();
            }

            match state {
                LoopState::Recording => {
                    let n = self.recorder.pop_slice(&mut self.recorded[..frames]);
                    if n > 0 {
                        self.stack.record_first_layer(&self.recorded[..n]);
                    }
                }
                LoopState::Looping | LoopState::Overdubbing => {
                    let loop_out = &mut self.loop_out[..frames];
                    if state == LoopState::Overdubbing {
                        let recorded = &mut self.recorded[..frames];
                        let n = self.recorder.pop_slice(recorded);
                        // A short read means the input fell behind;
                        // record the gap as silence rather than
                        // shifting everything after it out of time.
                        recorded[n..].fill(0);
                        let gain = self.control.volume_pct();
                        self.stack.read_mixed_with_overdub(loop_out, recorded, gain);
                    } else {
                        self.stack.read_mixed(loop_out, self.control.volume_pct());
                    }
                    mix_add(dry, loop_out);
                }
                // Arming captures nothing - the pre-roll is still
                // counting down on the UI thread.
                LoopState::Idle | LoopState::Stopped | LoopState::Arming => {}
            }

            duplicate_mono_to_channels(dry, self.channels, out);
        }

        self.control.publish_telemetry(
            self.stack.recorded_len(),
            self.stack.play_pos(),
            self.stack.layer_count(),
        );
    }
}

/// Both halves and the rings between them, built as one piece because
/// that is where the recorded signal's alignment against the monitored
/// signal is decided - see the prefill below.
struct AudioPath {
    input: InputPath,
    output: OutputPath,
}

/// Everything downstream of the device: no cpal types, so a test can
/// build one and push buffers through it.
fn build_audio_path(
    control: &Arc<SharedControl>,
    settings: &AppConfig,
    sample_rate: u32,
    channels: u16,
    restored: &[Vec<i32>],
) -> (AudioPath, HeapCons<i32>) {
    // Just enough headroom between the callbacks to absorb timing jitter,
    // not a deliberate monitoring delay. Everything below is mono.
    let latency_frames = (settings.latency_ms as usize * sample_rate as usize) / 1_000;

    // Dry passthrough bridge.
    let (mut passthrough_tx, passthrough_rx) = HeapRb::<i32>::new(latency_frames * 2).split();
    for _ in 0..latency_frames {
        passthrough_tx.try_push(0).unwrap();
    }

    // Feeds captured samples to the output path, which owns the layer
    // stack.
    let (recorder_tx, recorder_rx) = HeapRb::<i32>::new(latency_frames * 2).split();

    // The same samples again, for the copy the UI thread keeps so that
    // the loop can be saved.
    let (capture_tx, capture_rx) =
        HeapRb::<i32>::new(CAPTURE_BUFFER_SECONDS * sample_rate as usize).split();

    let loop_capacity = settings.max_loop_secs as usize * sample_rate as usize;
    let mut stack = LoopStack::new(loop_capacity, settings.max_layers as usize);
    for layer in restored {
        let added = stack.add_layer(layer);
        // Anything `loop_mirror::load` handed back fits by construction.
        // Dropping one quietly here would leave the UI thread's copy
        // holding layers that aren't playing.
        debug_assert!(added, "a restored layer should fit the settings it was loaded for");
    }

    let path = AudioPath {
        input: InputPath {
            control: Arc::clone(control),
            channels,
            input_channel: settings.input_channel,
            scratch: vec![0i32; SCRATCH_CAPACITY],
            passthrough: passthrough_tx,
            recorder: recorder_tx,
            capture: capture_tx,
        },
        output: OutputPath {
            control: Arc::clone(control),
            channels,
            dry: vec![0i32; SCRATCH_CAPACITY],
            loop_out: vec![0i32; SCRATCH_CAPACITY],
            recorded: vec![0i32; SCRATCH_CAPACITY],
            passthrough: passthrough_rx,
            recorder: recorder_rx,
            stack,
            previous_state: LoopState::Idle,
        },
    };
    (path, capture_rx)
}

/// Opens the device named in `settings` for input and output. Only its
/// chosen input channel is captured, treated as mono and duplicated
/// across every output channel; live input always passes through,
/// recording/looping follows `control`. `LoopStack` lives only inside the
/// output path, so nothing here needs a lock.
/// `restored` seeds the layer stack with a loop saved earlier, already
/// held to these settings by `loop_mirror::load`.
pub fn build_looper_streams(
    control: Arc<SharedControl>,
    settings: &AppConfig,
    restored: &[Vec<i32>],
) -> Result<LooperStreams, String> {
    let (device, config) = open_device_and_config(&settings.device_name, settings.sample_rate)?;
    let sample_rate = config.sample_rate;
    let channels = config.channels;
    if settings.input_channel >= channels {
        return Err(format!(
            "input channel {} is out of range (device has {channels} channel(s))",
            settings.input_channel + 1
        ));
    }

    let (path, captured) = build_audio_path(&control, settings, sample_rate, channels, restored);
    let AudioPath {
        mut input,
        mut output,
    } = path;

    let input_stream = device
        .build_input_stream(
            config.clone(),
            move |data: &[i32], _: &cpal::InputCallbackInfo| input.process(data),
            stream_err_fn,
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))?;

    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| output.process(data),
            stream_err_fn,
            None,
        )
        .map_err(|e| format!("failed to build output stream: {e}"))?;

    input_stream
        .play()
        .map_err(|e| format!("failed to start input stream: {e}"))?;
    output_stream
        .play()
        .map_err(|e| format!("failed to start output stream: {e}"))?;

    Ok(LooperStreams {
        input: input_stream,
        output: output_stream,
        sample_rate,
        captured,
    })
}

/// Layer bookkeeping that has to happen exactly when the state changes
/// rather than on every callback: fixing the loop length once the first
/// recording ends, and opening or closing an overdub layer.
fn apply_state_change(
    stack: &mut LoopStack,
    control: &SharedControl,
    from: LoopState,
    to: LoopState,
) {
    match from {
        LoopState::Recording => stack.finish_first_layer(),
        LoopState::Overdubbing => stack.finish_overdub(),
        _ => {}
    }
    match to {
        LoopState::Recording => stack.begin_first_layer(),
        LoopState::Overdubbing => {
            if stack.begin_overdub() {
                // Where this take starts, for the UI thread's copy: it
                // can't see the playback position the layer was aligned
                // to otherwise.
                control.publish_take_start(stack.play_pos());
            }
        }
        _ => {}
    }
}

fn stream_err_fn(err: cpal::Error) {
    eprintln!("stream error: {err}");
}

/// Adds `loop_signal` onto `dry` in place, saturating so a loud loop can't
/// wrap the live signal into its opposite sign.
fn mix_add(dry: &mut [i32], loop_signal: &[i32]) {
    for (d, l) in dry.iter_mut().zip(loop_signal.iter()) {
        *d = d.saturating_add(*l);
    }
}

/// Duplicates mono across every output channel, so a single input is
/// centered rather than coming out one side only.
fn duplicate_mono_to_channels(mono: &[i32], channels: u16, out: &mut [i32]) {
    for (frame_out, &sample) in out.chunks_exact_mut(channels as usize).zip(mono.iter()) {
        frame_out.fill(sample);
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
