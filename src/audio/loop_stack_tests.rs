use super::*;

/// A stack holding one finished layer of `samples`, ready at the top of
/// the loop.
fn with_first_layer(samples: &[i32]) -> LoopStack {
    let mut stack = LoopStack::new(16);
    stack.begin_first_layer();
    stack.record_first_layer(samples);
    stack.finish_first_layer();
    stack
}

#[test]
fn starts_empty() {
    let stack = LoopStack::new(8);
    assert!(stack.is_empty());
    assert_eq!(stack.recorded_len(), 0);
    assert_eq!(stack.layer_count(), 0);
}

#[test]
fn empty_stack_reads_silence() {
    let mut stack = LoopStack::new(8);
    let mut out = [7; 4];
    stack.read_mixed(&mut out, 100);
    assert_eq!(out, [0; 4]);
}

#[test]
fn first_layer_records_and_plays_back() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    assert_eq!(stack.recorded_len(), 4);
    assert_eq!(stack.layer_count(), 1);

    let mut out = [0; 4];
    stack.read_mixed(&mut out, 100);
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn first_layer_stops_at_capacity() {
    let mut stack = LoopStack::new(4);
    stack.begin_first_layer();
    assert_eq!(stack.record_first_layer(&[1, 2, 3, 4, 5, 6]), 4);
    stack.finish_first_layer();
    assert_eq!(stack.recorded_len(), 4);
}

#[test]
fn playback_wraps_at_recorded_length_not_capacity() {
    let mut stack = with_first_layer(&[1, 2, 3]);
    let mut out = [0; 7];
    stack.read_mixed(&mut out, 100);
    assert_eq!(out, [1, 2, 3, 1, 2, 3, 1]);
}

#[test]
fn playback_position_continues_across_calls() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);

    let mut first = [0; 3];
    stack.read_mixed(&mut first, 100);
    assert_eq!(first, [1, 2, 3]);
    assert_eq!(stack.play_pos(), 3);

    let mut second = [0; 3];
    stack.read_mixed(&mut second, 100);
    assert_eq!(second, [4, 1, 2]);
}

#[test]
fn clear_resets_everything_without_reallocating() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    let capacity = stack.capacity();

    stack.clear();

    assert!(stack.is_empty());
    assert_eq!(stack.layer_count(), 0);
    assert_eq!(stack.play_pos(), 0);
    assert_eq!(stack.capacity(), capacity);

    let mut out = [9; 4];
    stack.read_mixed(&mut out, 100);
    assert_eq!(out, [0; 4]);
}

#[test]
fn overdub_sums_onto_the_layer_below() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    assert!(stack.begin_overdub());

    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[10, 20, 30, 40], 100);
    stack.finish_overdub();
    assert_eq!(stack.layer_count(), 2);

    let mut played = [0; 4];
    stack.read_mixed(&mut played, 100);
    assert_eq!(played, [11, 22, 33, 44]);
}

#[test]
fn overdub_is_not_echoed_on_the_pass_it_is_recorded() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    stack.begin_overdub();

    // Only the layer below comes out - the player is already hearing
    // what they're playing, live.
    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[10, 20, 30, 40], 100);
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn overdub_only_contributes_where_it_was_recorded() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    stack.begin_overdub();

    // Two samples of a four-sample loop, then stopped.
    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[10, 20], 100);
    stack.finish_overdub();

    // Picks up mid-loop: positions 2 and 3 were never overdubbed.
    let mut played = [0; 4];
    stack.read_mixed(&mut played, 100);
    assert_eq!(played, [3, 4, 11, 22]);
}

#[test]
fn overdub_aligns_to_the_position_it_started_at() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);

    let mut skipped = [0; 2];
    stack.read_mixed(&mut skipped, 100);

    stack.begin_overdub();
    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[100, 200], 100);
    stack.finish_overdub();

    let mut played = [0; 4];
    stack.read_mixed(&mut played, 100);
    assert_eq!(played, [1, 2, 103, 204]);
}

#[test]
fn a_second_overdub_pass_sums_into_the_same_layer() {
    let mut stack = with_first_layer(&[0, 0]);
    stack.begin_overdub();

    // Two full passes of a two-sample loop.
    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[5, 5, 7, 7], 100);
    // Pass one comes back around during pass two.
    assert_eq!(out, [0, 0, 5, 5]);
    stack.finish_overdub();

    let mut played = [0; 2];
    stack.read_mixed(&mut played, 100);
    assert_eq!(played, [12, 12]);
}

#[test]
fn refuses_to_overdub_an_empty_stack() {
    let mut stack = LoopStack::new(8);
    assert!(!stack.begin_overdub());
}

#[test]
fn refuses_to_overdub_once_full() {
    let mut stack = with_first_layer(&[1, 2]);
    for _ in 1..MAX_LAYERS {
        assert!(stack.begin_overdub());
        let mut out = [0; 2];
        stack.read_mixed_with_overdub(&mut out, &[1, 1], 100);
        stack.finish_overdub();
    }

    assert!(stack.is_full());
    assert!(!stack.begin_overdub());
}

#[test]
fn an_overdub_take_with_nothing_in_it_is_dropped() {
    let mut stack = with_first_layer(&[1, 2]);
    stack.begin_overdub();
    stack.finish_overdub();
    assert_eq!(stack.layer_count(), 1);
}

#[test]
fn remove_last_layer_drops_only_the_newest() {
    let mut stack = with_first_layer(&[1, 2]);
    stack.begin_overdub();
    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[10, 20], 100);
    stack.finish_overdub();

    let mut both = [0; 2];
    stack.read_mixed(&mut both, 100);
    assert_eq!(both, [11, 22]);

    stack.remove_last_layer();
    assert_eq!(stack.layer_count(), 1);

    let mut left = [0; 2];
    stack.read_mixed(&mut left, 100);
    assert_eq!(left, [1, 2]);
}

#[test]
fn removing_the_only_layer_empties_the_loop() {
    let mut stack = with_first_layer(&[1, 2]);
    stack.remove_last_layer();
    assert!(stack.is_empty());
    assert_eq!(stack.recorded_len(), 0);
}

#[test]
fn loop_volume_scales_every_layer_together() {
    let mut stack = with_first_layer(&[100, 200]);

    let mut half = [0; 2];
    stack.read_mixed(&mut half, 50);
    assert_eq!(half, [50, 100]);

    let mut silent = [0; 2];
    stack.read_mixed(&mut silent, 0);
    assert_eq!(silent, [0, 0]);

    let mut doubled = [0; 2];
    stack.read_mixed(&mut doubled, 200);
    assert_eq!(doubled, [200, 400]);
}

#[test]
fn stacked_layers_stay_recoverable_instead_of_clipping() {
    // Four takes this loud overflow a sample when summed, so a 32-bit
    // mix would clip them away for good.
    let loud = i32::MAX / 2;
    let mut stack = with_first_layer(&[loud, loud]);
    for _ in 1..MAX_LAYERS {
        stack.begin_overdub();
        let mut out = [0; 2];
        stack.read_mixed_with_overdub(&mut out, &[loud, loud], 100);
        stack.finish_overdub();
    }

    // At unity it saturates, as it must - there's nowhere left to put it.
    let mut hot = [0; 2];
    stack.read_mixed(&mut hot, 100);
    assert_eq!(hot, [i32::MAX, i32::MAX]);

    // Turning the loop volume down still recovers the true sum, which
    // is what summing in 64-bit buys.
    let mut turned_down = [0; 2];
    stack.read_mixed(&mut turned_down, 25);
    assert_eq!(turned_down, [loud, loud]);
}

#[test]
fn recorded_length_grows_while_the_first_take_runs() {
    let mut stack = LoopStack::new(16);
    stack.begin_first_layer();
    assert_eq!(stack.recorded_len(), 0);

    // The elapsed time shown while recording reads this, so it has to
    // grow with the take rather than waiting for the loop length to be
    // fixed at the end.
    stack.record_first_layer(&[1, 2, 3]);
    assert_eq!(stack.recorded_len(), 3);
    stack.record_first_layer(&[4, 5]);
    assert_eq!(stack.recorded_len(), 5);

    stack.finish_first_layer();
    assert_eq!(stack.recorded_len(), 5);
}

#[test]
fn an_overdub_does_not_change_the_recorded_length() {
    let mut stack = with_first_layer(&[1, 2]);
    stack.begin_overdub();

    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[3, 4], 100);
    assert_eq!(stack.recorded_len(), 2, "the loop length is already fixed");
}
