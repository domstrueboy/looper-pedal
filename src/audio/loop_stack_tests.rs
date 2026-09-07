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
    assert_eq!(stack.len(), 0);
    assert_eq!(stack.layer_count(), 0);
}

#[test]
fn empty_stack_reads_silence() {
    let mut stack = LoopStack::new(8);
    let mut out = [7; 4];
    stack.read_mixed(&mut out);
    assert_eq!(out, [0; 4]);
}

#[test]
fn first_layer_records_and_plays_back() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    assert_eq!(stack.len(), 4);
    assert_eq!(stack.layer_count(), 1);

    let mut out = [0; 4];
    stack.read_mixed(&mut out);
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn first_layer_stops_at_capacity() {
    let mut stack = LoopStack::new(4);
    stack.begin_first_layer();
    assert_eq!(stack.record_first_layer(&[1, 2, 3, 4, 5, 6]), 4);
    stack.finish_first_layer();
    assert_eq!(stack.len(), 4);
}

#[test]
fn playback_wraps_at_recorded_length_not_capacity() {
    let mut stack = with_first_layer(&[1, 2, 3]);
    let mut out = [0; 7];
    stack.read_mixed(&mut out);
    assert_eq!(out, [1, 2, 3, 1, 2, 3, 1]);
}

#[test]
fn playback_position_continues_across_calls() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);

    let mut first = [0; 3];
    stack.read_mixed(&mut first);
    assert_eq!(first, [1, 2, 3]);
    assert_eq!(stack.play_pos(), 3);

    let mut second = [0; 3];
    stack.read_mixed(&mut second);
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
    stack.read_mixed(&mut out);
    assert_eq!(out, [0; 4]);
}

#[test]
fn overdub_sums_onto_the_layer_below() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    assert!(stack.begin_overdub());

    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[10, 20, 30, 40]);
    stack.finish_overdub();
    assert_eq!(stack.layer_count(), 2);

    let mut played = [0; 4];
    stack.read_mixed(&mut played);
    assert_eq!(played, [11, 22, 33, 44]);
}

#[test]
fn overdub_is_not_echoed_on_the_pass_it_is_recorded() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    stack.begin_overdub();

    // Only the layer below comes out - the player is already hearing
    // what they're playing, live.
    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[10, 20, 30, 40]);
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn overdub_only_contributes_where_it_was_recorded() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);
    stack.begin_overdub();

    // Two samples of a four-sample loop, then stopped.
    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[10, 20]);
    stack.finish_overdub();

    // Picks up mid-loop: positions 2 and 3 were never overdubbed.
    let mut played = [0; 4];
    stack.read_mixed(&mut played);
    assert_eq!(played, [3, 4, 11, 22]);
}

#[test]
fn overdub_aligns_to_the_position_it_started_at() {
    let mut stack = with_first_layer(&[1, 2, 3, 4]);

    let mut skipped = [0; 2];
    stack.read_mixed(&mut skipped);

    stack.begin_overdub();
    let mut out = [0; 2];
    stack.read_mixed_with_overdub(&mut out, &[100, 200]);
    stack.finish_overdub();

    let mut played = [0; 4];
    stack.read_mixed(&mut played);
    assert_eq!(played, [1, 2, 103, 204]);
}

#[test]
fn a_second_overdub_pass_sums_into_the_same_layer() {
    let mut stack = with_first_layer(&[0, 0]);
    stack.begin_overdub();

    // Two full passes of a two-sample loop.
    let mut out = [0; 4];
    stack.read_mixed_with_overdub(&mut out, &[5, 5, 7, 7]);
    // Pass one comes back around during pass two.
    assert_eq!(out, [0, 0, 5, 5]);
    stack.finish_overdub();

    let mut played = [0; 2];
    stack.read_mixed(&mut played);
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
        stack.read_mixed_with_overdub(&mut out, &[1, 1]);
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
    stack.read_mixed_with_overdub(&mut out, &[10, 20]);
    stack.finish_overdub();

    let mut both = [0; 2];
    stack.read_mixed(&mut both);
    assert_eq!(both, [11, 22]);

    stack.remove_last_layer();
    assert_eq!(stack.layer_count(), 1);

    let mut left = [0; 2];
    stack.read_mixed(&mut left);
    assert_eq!(left, [1, 2]);
}

#[test]
fn removing_the_only_layer_empties_the_loop() {
    let mut stack = with_first_layer(&[1, 2]);
    stack.remove_last_layer();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
}
