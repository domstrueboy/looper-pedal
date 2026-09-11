// The core works in f32 with full scale at +/-1.0; the devices it opens
// and the WAVs it saves work in 32-bit integers. One module owns the
// conversion between the two, so the file edge and the device edge can't
// drift apart and disagree about how loud a sample is.

/// 2^31, so full-scale `i32` lands on +/-1.0. The same convention `cpal`
/// and `dasp` use, which is what lets a device's own conversion agree
/// with ours rather than being half a step out.
const SCALE: f32 = 2_147_483_648.0;

/// A stored 32-bit sample, as the core sees it.
///
/// Dividing by a power of two is exact, so the only rounding is the cast
/// itself: `f32` carries 24 significant bits against `i32`'s 31. A 24-bit
/// converter (which is what the interfaces this runs on are) produces
/// samples that fit exactly, so a saved loop reloads bit for bit.
pub fn from_pcm32(sample: i32) -> f32 {
    sample as f32 / SCALE
}

/// Back again, for a device or a file.
///
/// Rounds rather than truncating: a cast toward zero turns a value that
/// landed a hair under `k` into `k - 1`, which is half a step of bias on
/// every sample rather than a rounding error that averages out.
///
/// This is the one place in the app where clipping happens. Everything
/// upstream carries the full sum - four stacked layers can exceed full
/// scale, and clamping that away earlier would make the loudness
/// unrecoverable by the volume control. The float-to-int cast saturates
/// (and maps NaN to zero), which is exactly what's wanted: `+1.0` scales
/// to one past `i32::MAX` and lands on it, `-1.0` maps exactly onto
/// `i32::MIN`.
pub fn to_pcm32(sample: f32) -> i32 {
    (sample * SCALE).round() as i32
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
