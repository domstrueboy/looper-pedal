//! Opening an ASIO device and wiring it to the audio path.
//!
//! Everything `cpal` touches lives here, which is all that stands
//! between this app and another operating system. It moves into a
//! backend behind `looper-hal`'s traits next; until then it is the one
//! place that knows what a device is.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapCons;

use looper_core::audio::engine::{AudioPath, SCRATCH_CAPACITY, build_audio_path};
use looper_core::audio::shared_control::SharedControl;
use looper_core::config::AppConfig;
use looper_core::sample;

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

/// The live streams, plus the channel the UI thread reads captured
/// samples from - it keeps its own copy of the loop, since the layer
/// stack itself is out of reach inside the output callback.
pub struct LooperStreams {
    pub input: cpal::Stream,
    pub output: cpal::Stream,
    pub sample_rate: u32,
    pub captured: HeapCons<f32>,
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
    restored: &[Vec<f32>],
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

    // The audio path works in f32; this device is i32 (asserted above).
    // Converting here, either side of the callback, is what keeps the
    // format assumption at the edge instead of threaded through the path
    // - and it's the job the hardware layer takes over when the backends
    // land, at which point a device that isn't i32 becomes openable.
    let block = SCRATCH_CAPACITY * channels as usize;
    let mut input_scratch = vec![0.0f32; block];
    let mut output_scratch = vec![0.0f32; block];

    let input_stream = device
        .build_input_stream(
            config.clone(),
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                for chunk in data.chunks(block) {
                    let converted = &mut input_scratch[..chunk.len()];
                    for (slot, &raw) in converted.iter_mut().zip(chunk) {
                        *slot = sample::from_pcm32(raw);
                    }
                    input.process(converted);
                }
            },
            stream_err_fn,
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))?;

    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                for chunk in data.chunks_mut(block) {
                    let rendered = &mut output_scratch[..chunk.len()];
                    output.process(rendered);
                    for (slot, &value) in chunk.iter_mut().zip(rendered.iter()) {
                        *slot = sample::to_pcm32(value);
                    }
                }
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

    Ok(LooperStreams {
        input: input_stream,
        output: output_stream,
        sample_rate,
        captured,
    })
}

fn stream_err_fn(err: cpal::Error) {
    eprintln!("stream error: {err}");
}
