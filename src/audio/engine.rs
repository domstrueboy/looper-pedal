use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};

use super::loop_buffer::LoopBuffer;
use super::state_machine::LoopState;

const MAX_LOOP_SECONDS: f32 = 60.0;
const SCRATCH_CAPACITY: usize = 32768;

/// Lock-free relay of state changes into the audio callback. The
/// `LoopStateMachine` itself stays single-owner on the UI thread (see
/// `main.rs`) - only the resulting state value and a one-shot clear flag
/// cross the thread boundary, both via atomics, never a lock.
pub struct SharedControl {
    state: AtomicU8,
    clear_requested: AtomicBool,
}

impl SharedControl {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(LoopState::Idle as u8),
            clear_requested: AtomicBool::new(false),
        }
    }

    pub fn publish_state(&self, state: LoopState) {
        self.state.store(state as u8, Ordering::Release);
    }

    pub fn request_clear(&self) {
        self.clear_requested.store(true, Ordering::Release);
    }

    fn load_state(&self) -> LoopState {
        match self.state.load(Ordering::Acquire) {
            0 => LoopState::Idle,
            1 => LoopState::Recording,
            2 => LoopState::Looping,
            _ => LoopState::Stopped,
        }
    }

    fn take_clear_request(&self) -> bool {
        self.clear_requested.swap(false, Ordering::AcqRel)
    }
}

pub fn list_asio_devices() {
    let host = cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable");

    println!("ASIO devices:");
    for device in host.devices().expect("failed to enumerate ASIO devices") {
        println!("  - {device}");
    }
}

/// Opens the (single) ASIO device for both input and output. Input is
/// always passed through live; recording/looping is driven by `control`,
/// published from the UI thread. `LoopBuffer` lives exclusively inside the
/// output callback (never shared), so no locks are needed anywhere here.
pub fn build_looper_streams(control: Arc<SharedControl>) -> (cpal::Stream, cpal::Stream) {
    let host = cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable");
    let device = host
        .devices()
        .expect("failed to enumerate ASIO devices")
        .next()
        .expect("no ASIO device found");

    println!("Using device: {device}");

    let input_config = device
        .default_input_config()
        .expect("failed to get default input config");
    let output_config = device
        .default_output_config()
        .expect("failed to get default output config");
    assert_eq!(
        input_config.sample_format(),
        output_config.sample_format(),
        "input and output must share a sample format"
    );
    assert_eq!(
        input_config.sample_format(),
        cpal::SampleFormat::I32,
        "expected an i32 ASIO stream (the Audient iD4 MkII's native format)"
    );

    let config: cpal::StreamConfig = input_config.into();
    println!(
        "Stream config: {} Hz, {} channel(s), buffer size: {:?}",
        config.sample_rate, config.channels, config.buffer_size
    );

    // Headroom between the input and output callbacks, just enough to
    // absorb callback-timing jitter (not a deliberate monitoring delay).
    const LATENCY_MS: f32 = 8.0;
    let latency_frames = (LATENCY_MS / 1_000.0) * config.sample_rate as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;

    // Dry passthrough bridge (unchanged behavior from step 3).
    let passthrough_ring = HeapRb::<i32>::new(latency_samples * 2);
    let (mut passthrough_tx, mut passthrough_rx) = passthrough_ring.split();
    for _ in 0..latency_samples {
        passthrough_tx.try_push(0).unwrap();
    }

    // Feeds captured samples from the input callback into the output
    // callback, which owns the LoopBuffer exclusively while Recording.
    let recorder_ring = HeapRb::<i32>::new(latency_samples * 2);
    let (mut recorder_tx, mut recorder_rx) = recorder_ring.split();

    let loop_capacity =
        (MAX_LOOP_SECONDS * config.sample_rate as f32) as usize * config.channels as usize;
    let mut loop_buffer = LoopBuffer::new(loop_capacity);

    let input_control = Arc::clone(&control);
    let input_stream = device
        .build_input_stream(
            config.clone(),
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                if passthrough_tx.push_slice(data) < data.len() {
                    eprintln!("output stream fell behind: try increasing latency");
                }
                if input_control.load_state() == LoopState::Recording {
                    recorder_tx.push_slice(data);
                }
            },
            stream_err_fn,
            None,
        )
        .expect("failed to build input stream");

    let output_control = control;
    let mut scratch = vec![0i32; SCRATCH_CAPACITY];
    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                let read = passthrough_rx.pop_slice(data);
                if read < data.len() {
                    data[read..].fill(0);
                    eprintln!("input stream fell behind: try increasing latency");
                }

                if output_control.take_clear_request() {
                    loop_buffer.clear();
                }

                match output_control.load_state() {
                    LoopState::Recording => {
                        let n = recorder_rx.pop_slice(&mut scratch[..data.len()]);
                        if n > 0 {
                            loop_buffer.write(&scratch[..n]);
                        }
                    }
                    LoopState::Looping => {
                        let loop_out = &mut scratch[..data.len()];
                        loop_buffer.read_looped(loop_out);
                        for (o, l) in data.iter_mut().zip(loop_out.iter()) {
                            *o = o.saturating_add(*l);
                        }
                    }
                    LoopState::Idle | LoopState::Stopped => {}
                }
            },
            stream_err_fn,
            None,
        )
        .expect("failed to build output stream");

    input_stream.play().expect("failed to start input stream");
    output_stream
        .play()
        .expect("failed to start output stream");

    (input_stream, output_stream)
}

fn stream_err_fn(err: cpal::Error) {
    eprintln!("stream error: {err}");
}
