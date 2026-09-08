use super::*;

use ringbuf::traits::{Producer, Split};
use ringbuf::{HeapProd, HeapRb};

use crate::audio::loop_stack::LoopStack;

const RATE: u32 = 44_100;

fn new_mirror() -> (HeapProd<i32>, LoopMirror) {
    let (producer, consumer) = HeapRb::<i32>::new(1024).split();
    (producer, LoopMirror::new(consumer, RATE, Vec::new()))
}

/// One take, driven the way a frame loop would: the state changes, the
/// input callback delivers samples, the state changes back. The extra
/// tick is the frame on which the loop length settles.
fn record(
    input: &mut HeapProd<i32>,
    mirror: &mut LoopMirror,
    state: LoopState,
    samples: &[i32],
    loop_len: usize,
    take_start: usize,
) -> bool {
    mirror.tick(state, loop_len, take_start);
    input.push_slice(samples);
    mirror.tick(state, loop_len, take_start);
    mirror.tick(LoopState::Looping, loop_len, take_start);
    mirror.tick(LoopState::Looping, loop_len, take_start)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("looper-pedal-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_first_take_becomes_the_only_layer() {
    let (mut input, mut mirror) = new_mirror();

    let out_of_date = record(
        &mut input,
        &mut mirror,
        LoopState::Recording,
        &[1, 2, 3, 4],
        4,
        0,
    );

    assert!(out_of_date, "a new take needs saving");
    assert_eq!(mirror.layers().len(), 1);
    assert_eq!(mirror.layers(), vec![vec![1, 2, 3, 4]]);
}

#[test]
fn an_overdub_lands_where_it_began() {
    let (mut input, mut mirror) = new_mirror();
    record(
        &mut input,
        &mut mirror,
        LoopState::Recording,
        &[1, 2, 3, 4],
        4,
        0,
    );

    // Started two samples into the loop, and only two long.
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[9, 8],
        4,
        2,
    );

    assert_eq!(mirror.layers().len(), 2);
    assert_eq!(mirror.layers()[1], vec![0, 0, 9, 8]);
}

#[test]
fn a_second_pass_sums_into_the_same_layer() {
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[0; 4], 4, 0);

    // Six samples over a four-sample loop: the last two come back round.
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[1, 1, 1, 1, 2, 2],
        4,
        0,
    );

    assert_eq!(mirror.layers()[1], vec![3, 3, 1, 1]);
}

#[test]
fn a_laid_out_take_matches_what_the_layer_stack_plays() {
    // The audio thread writes overdubs sample by sample as it plays;
    // this lays the same take out in one go. The two rules are written
    // separately, so this is what keeps them honest.
    let base = [10, 20, 30, 40];
    let take = [1, 2, 3, 4, 5, 6];
    let start = 2;

    let mut stack = LoopStack::new(16, 4);
    stack.begin_first_layer();
    stack.record_first_layer(&base);
    stack.finish_first_layer();

    let mut skipped = vec![0; start];
    stack.read_mixed(&mut skipped, 100);
    stack.begin_overdub();
    let mut heard = vec![0; take.len()];
    stack.read_mixed_with_overdub(&mut heard, &take, 100);
    stack.finish_overdub();

    // The overdub ended back at the top of the loop.
    let mut played = vec![0; base.len()];
    stack.read_mixed(&mut played, 100);

    let (mut input, mut mirror) = new_mirror();
    record(
        &mut input,
        &mut mirror,
        LoopState::Recording,
        &base,
        base.len(),
        0,
    );
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &take,
        base.len(),
        start,
    );

    let mirrored = &mirror.layers()[1];
    let summed: Vec<i32> = base
        .iter()
        .zip(mirrored)
        .map(|(base, layer)| base + layer)
        .collect();
    assert_eq!(summed, played, "mirrored layer should sum to what plays");
}

#[test]
fn recording_again_replaces_the_loop() {
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[1, 2], 2, 0);
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[3, 4],
        2,
        0,
    );
    assert_eq!(mirror.layers().len(), 2);

    record(
        &mut input,
        &mut mirror,
        LoopState::Recording,
        &[5, 6, 7],
        3,
        0,
    );

    assert_eq!(mirror.layers().len(), 1);
    assert_eq!(mirror.layers(), vec![vec![5, 6, 7]]);
}

#[test]
fn removing_and_clearing_follow_the_stack() {
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[1, 2], 2, 0);
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[3, 4],
        2,
        0,
    );

    mirror.remove_last_layer();
    assert_eq!(mirror.layers().len(), 1);

    mirror.clear();
    assert_eq!(mirror.layers().len(), 0);
    assert!(mirror.layers().is_empty());
}

#[test]
fn samples_arriving_with_no_take_open_are_dropped() {
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[1, 2], 2, 0);

    // Stragglers from the take that just ended.
    input.push_slice(&[99, 99]);
    mirror.tick(LoopState::Looping, 2, 0);

    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[3, 4],
        2,
        0,
    );
    assert_eq!(mirror.layers()[1], vec![3, 4], "no stragglers mixed in");
}

#[test]
fn a_saved_loop_comes_back_layer_for_layer() {
    let directory = temp_dir("round-trip");
    let (mut input, mut mirror) = new_mirror();
    record(
        &mut input,
        &mut mirror,
        LoopState::Recording,
        &[1, 2, 3],
        3,
        0,
    );
    record(
        &mut input,
        &mut mirror,
        LoopState::Overdubbing,
        &[4, 5, 6],
        3,
        0,
    );

    mirror.save(&directory).expect("saves");
    let loaded = load(&directory, RATE);

    assert_eq!(loaded, vec![vec![1, 2, 3], vec![4, 5, 6]]);

    delete(&directory);
    assert!(load(&directory, RATE).is_empty(), "deleted");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_loop_recorded_at_another_rate_is_not_loaded() {
    let directory = temp_dir("wrong-rate");
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[1, 2], 2, 0);
    mirror.save(&directory).expect("saves");

    // No resampling, so a rate change means the loop is unusable.
    assert!(load(&directory, 48_000).is_empty());
    assert_eq!(load(&directory, RATE).len(), 1);

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_gap_in_what_arrived_stops_the_loop_being_saved() {
    let directory = temp_dir("lost");
    let (mut input, mut mirror) = new_mirror();
    record(&mut input, &mut mirror, LoopState::Recording, &[1, 2], 2, 0);

    mirror.note_lost_samples();

    // Better no save than one that doesn't match what was played.
    assert!(mirror.save(&directory).is_err());
    assert!(load(&directory, RATE).is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}
