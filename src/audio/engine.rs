use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};

pub fn list_asio_devices() {
    let host = cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable");

    println!("ASIO devices:");
    for device in host.devices().expect("failed to enumerate ASIO devices") {
        println!("  - {device}");
    }
}

/// Opens the (single) ASIO device for both input and output and wires input
/// straight through to output via a small ring buffer. No loop logic yet.
pub fn build_passthrough_streams() -> (cpal::Stream, cpal::Stream) {
    let host = cpal::host_from_id(cpal::HostId::Asio).expect("ASIO host unavailable");
    let device = host
        .devices()
        .expect("failed to enumerate ASIO devices")
        .next()
        .expect("no ASIO device found");

    println!("Passthrough using device: {device}");

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

    let ring = HeapRb::<i32>::new(latency_samples * 2);
    let (mut producer, mut consumer) = ring.split();
    for _ in 0..latency_samples {
        producer.try_push(0).unwrap();
    }

    let input_stream = device
        .build_input_stream(
            config.clone(),
            move |data: &[i32], _: &cpal::InputCallbackInfo| {
                if producer.push_slice(data) < data.len() {
                    eprintln!("output stream fell behind: try increasing latency");
                }
            },
            stream_err_fn,
            None,
        )
        .expect("failed to build input stream");

    let output_stream = device
        .build_output_stream(
            config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                let read = consumer.pop_slice(data);
                if read < data.len() {
                    data[read..].fill(0);
                    eprintln!("input stream fell behind: try increasing latency");
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
