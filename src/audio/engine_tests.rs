use super::*;

#[test]
fn apply_gain_pct_100_is_unchanged() {
    let mut samples = [10, -20, 0];
    apply_gain_pct(&mut samples, 100);
    assert_eq!(samples, [10, -20, 0]);
}

#[test]
fn apply_gain_pct_0_is_silence() {
    let mut samples = [10, -20, 12345];
    apply_gain_pct(&mut samples, 0);
    assert_eq!(samples, [0, 0, 0]);
}

#[test]
fn apply_gain_pct_200_doubles() {
    let mut samples = [10, -20];
    apply_gain_pct(&mut samples, 200);
    assert_eq!(samples, [20, -40]);
}

#[test]
fn apply_gain_pct_saturates_instead_of_wrapping() {
    let mut samples = [i32::MAX, i32::MIN];
    apply_gain_pct(&mut samples, 200);
    assert_eq!(samples, [i32::MAX, i32::MIN]);
}

#[test]
fn mix_add_sums_dry_and_loop_signal() {
    let mut dry = [10, 20, 30];
    mix_add(&mut dry, &[1, 2, 3]);
    assert_eq!(dry, [11, 22, 33]);
}

#[test]
fn mix_add_saturates_instead_of_wrapping() {
    let mut dry = [i32::MAX, i32::MIN];
    mix_add(&mut dry, &[1, -1]);
    assert_eq!(dry, [i32::MAX, i32::MIN]);
}

#[test]
fn duplicate_mono_to_channels_fills_every_channel_with_the_same_sample() {
    let mono = [1, 2, 3];
    let mut out = [0; 6];
    duplicate_mono_to_channels(&mono, 2, &mut out);
    assert_eq!(out, [1, 1, 2, 2, 3, 3]);
}

#[test]
fn duplicate_mono_to_channels_handles_mono_output() {
    let mono = [5, 6];
    let mut out = [0; 2];
    duplicate_mono_to_channels(&mono, 1, &mut out);
    assert_eq!(out, [5, 6]);
}
