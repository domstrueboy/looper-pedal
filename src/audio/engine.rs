use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};

use super::loop_buffer::LoopBuffer;
use super::shared_control::SharedControl;
use super::state_machine::LoopState;

const MAX_LOOP_SECONDS: f32 = 60.0;
const SCRATCH_CAPACITY: usize = 32768;
const LATENCY_MS: f32 = 8.0;

// Candidate rates to offer in the settings picker; only ones the chosen
// device actually supports (per `contains_rate`) are shown.
const CANDIDATE_SAMPLE_RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

fn asio_host() -> cpal::Host {
    cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable")
}

pub fn available_asio_devices() -> Result<Vec<String>, String> {
    asio_host()
        .devices()
        .map_err(|e| format!("failed to enumerate ASIO devices: {e}"))
        .map(|devices| devices.map(|d| d.to_string()).collect())
}

fn find_device(device_name: &str) -> Result<cpal::Device, String> {
    asio_host()
        .devices()
        .map_err(|e| format!("failed to enumerate ASIO devices: {e}"))?
        .find(|d| d.to_string() == device_name)
        .ok_or_else(|| format!("ASIO device '{device_name}' not found"))
}

/// Sample rates from `CANDIDATE_SAMPLE_RATES` that `device_name` actually
/// supports for the i32 format this project is built around.
pub fn supported_sample_rates(device_name: &str) -> Result<Vec<u32>, String> {
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

/// Number of hardware input channels `device_name` exposes (e.g. 2 for the
/// Audient iD4 MkII's two combo inputs), so settings can offer a picker.
pub fn input_channel_count(device_name: &str) -> Result<u16, String> {
    let device = find_device(device_name)?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get default input config: {e}"))?;
    Ok(config.channels())
}

/// Finds `device_name` and negotiates an input/output config at
/// `sample_rate`, asserting the i32 format this project is built around
/// (the Audient iD4 MkII's native format). Returns a descriptive error
/// instead of panicking, since device/rate mismatches are user-recoverable
/// (pick a different one in settings) rather than programming errors.
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

    // `supported_output_configs()` lists a SEPARATE entry per channel
    // count (1, 2, 3, 4, ...) at each rate - we must match the device's
    // full channel count explicitly, or we'd silently pick the first
    // (lowest, e.g. mono) entry instead of opening all real channels.
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

/// Opens `device_name` at `sample_rate` for both input and output. Only
/// `input_channel` (0-indexed) is actually captured/recorded/looped - it's
/// treated as mono internally and duplicated equally across every output
/// channel, so a single guitar input is centered in both ears rather than
/// only coming out of one side. Input is always passed through live;
/// recording/looping is driven by `control`, published from the UI thread.
/// `LoopBuffer` lives exclusively inside the output callback (never
/// shared), so no locks are needed anywhere here.
pub fn build_looper_streams(
    control: Arc<SharedControl>,
    device_name: &str,
    sample_rate: u32,
    input_channel: u16,
) -> Result<(cpal::Stream, cpal::Stream, u32), String> {
    let (device, config) = open_device_and_config(device_name, sample_rate)?;
    let sample_rate = config.sample_rate;
    let channels = config.channels;
    if input_channel >= channels {
        return Err(format!(
            "input channel {} is out of range (device has {channels} channel(s))",
            input_channel + 1
        ));
    }

    // Headroom between the input and output callbacks, just enough to
    // absorb callback-timing jitter (not a deliberate monitoring delay).
    // Everything from here on is mono (one sample per frame).
    let latency_frames = ((LATENCY_MS / 1_000.0) * config.sample_rate as f32) as usize;

    // Dry passthrough bridge (unchanged behavior from step 3, now mono).
    let passthrough_ring = HeapRb::<i32>::new(latency_frames * 2);
    let (mut passthrough_tx, mut passthrough_rx) = passthrough_ring.split();
    for _ in 0..latency_frames {
        passthrough_tx.try_push(0).unwrap();
    }

    // Feeds captured samples from the input callback into the output
    // callback, which owns the LoopBuffer exclusively while Recording.
    let recorder_ring = HeapRb::<i32>::new(latency_frames * 2);
    let (mut recorder_tx, mut recorder_rx) = recorder_ring.split();

    let loop_capacity = (MAX_LOOP_SECONDS * config.sample_rate as f32) as usize;
    let mut loop_buffer = LoopBuffer::new(loop_capacity);

    let input_control = Arc::clone(&control);
    let mut input_scratch = vec![0i32; SCRATCH_CAPACITY];
    let input_stream = device
        .build_input_stream(
            config.clone(),
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                // Bound every chunk at SCRATCH_CAPACITY frames regardless of
                // how many frames the driver hands us in one callback - the
                // scratch buffers are fixed-size and this is a real-time
                // callback, so indexing past them would panic rather than
                // gracefully degrade. In practice a callback this large
                // never happens, but the guard is cheap.
                for chunk in data.chunks(SCRATCH_CAPACITY * channels as usize) {
                    let frames = chunk.len() / channels as usize;
                    for (i, frame) in chunk.chunks_exact(channels as usize).enumerate() {
                        input_scratch[i] = frame[input_channel as usize];
                    }
                    let mono = &input_scratch[..frames];

                    if passthrough_tx.push_slice(mono) < mono.len() {
                        input_control.note_output_underrun();
                    }
                    if input_control.load_state() == LoopState::Recording {
                        recorder_tx.push_slice(mono);
                    }
                }
            },
            stream_err_fn,
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))?;

    let output_control = control;
    let mut dry_scratch = vec![0i32; SCRATCH_CAPACITY];
    let mut loop_scratch = vec![0i32; SCRATCH_CAPACITY];
    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                // Same chunk-bounding as the input callback - see the
                // comment there.
                for out in data.chunks_mut(SCRATCH_CAPACITY * channels as usize) {
                    let frames = out.len() / channels as usize;
                    let dry = &mut dry_scratch[..frames];

                    let read = passthrough_rx.pop_slice(dry);
                    if read < frames {
                        dry[read..].fill(0);
                        output_control.note_input_underrun();
                    }

                    if output_control.take_clear_request() {
                        loop_buffer.clear();
                    }

                    match output_control.load_state() {
                        LoopState::Recording => {
                            let n = recorder_rx.pop_slice(&mut loop_scratch[..frames]);
                            if n > 0 {
                                loop_buffer.write(&loop_scratch[..n]);
                            }
                        }
                        LoopState::Looping => {
                            let loop_out = &mut loop_scratch[..frames];
                            loop_buffer.read_looped(loop_out);
                            for (d, l) in dry.iter_mut().zip(loop_out.iter()) {
                                *d = d.saturating_add(*l);
                            }
                        }
                        LoopState::Idle | LoopState::Stopped => {}
                    }

                    for (frame_out, &mono_sample) in
                        out.chunks_exact_mut(channels as usize).zip(dry.iter())
                    {
                        frame_out.fill(mono_sample);
                    }
                }

                output_control.publish_loop_progress(loop_buffer.len(), loop_buffer.play_pos());
            },
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

    Ok((input_stream, output_stream, sample_rate))
}

fn stream_err_fn(err: cpal::Error) {
    eprintln!("stream error: {err}");
}
