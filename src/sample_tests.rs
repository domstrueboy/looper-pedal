use super::*;

#[test]
fn silence_is_silence_either_way() {
    assert_eq!(from_pcm32(0), 0.0);
    assert_eq!(to_pcm32(0.0), 0);
}

#[test]
fn full_scale_maps_to_plus_and_minus_one() {
    assert_eq!(from_pcm32(i32::MIN), -1.0);
    assert_eq!(to_pcm32(-1.0), i32::MIN);
    // The positive end is a step short: +1.0 scales to one past
    // `i32::MAX`, which saturates back onto it. The asymmetry is two's
    // complement's, not ours.
    assert_eq!(to_pcm32(1.0), i32::MAX);
}

#[test]
fn a_24_bit_sample_round_trips_exactly() {
    // What a 24-bit converter actually delivers in a 32-bit container:
    // multiples of 256, which fit `f32`'s 24 significant bits with room
    // to spare. This is the promise that a saved loop reloads unchanged.
    for code in [256, -256, 0x0012_3400, -0x0012_3400, 0x4000_0000] {
        assert_eq!(to_pcm32(from_pcm32(code)), code, "code {code:#x}");
    }
}

#[test]
fn a_loop_hotter_than_full_scale_clips_instead_of_wrapping() {
    // Four stacked layers can sum past full scale. It has to arrive at
    // the converter as the loudest sample there is, never as its
    // opposite sign.
    assert_eq!(to_pcm32(2.0), i32::MAX);
    assert_eq!(to_pcm32(-2.0), i32::MIN);
    assert_eq!(to_pcm32(1_000.0), i32::MAX);
}

#[test]
fn rounding_does_not_bias_toward_zero() {
    // Just under one step: truncation would drop it to zero and take
    // half a step off every sample in the loop with it.
    let just_under = 0.9 / SCALE;
    assert_eq!(to_pcm32(just_under), 1);
    assert_eq!(to_pcm32(-just_under), -1);
}
