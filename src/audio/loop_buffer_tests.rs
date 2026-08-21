use super::*;

#[test]
fn write_then_read_back_matches() {
    let mut buf = LoopBuffer::new(16);
    let written = buf.write(&[1, 2, 3, 4]);
    assert_eq!(written, 4);
    assert_eq!(buf.len(), 4);

    let mut out = [0; 4];
    buf.read_looped(&mut out);
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn read_looped_wraps_around() {
    let mut buf = LoopBuffer::new(16);
    buf.write(&[1, 2, 3]);

    let mut out = [0; 7];
    buf.read_looped(&mut out);
    assert_eq!(out, [1, 2, 3, 1, 2, 3, 1]);
}

#[test]
fn playback_position_continues_across_calls() {
    let mut buf = LoopBuffer::new(16);
    buf.write(&[1, 2, 3]);

    let mut first = [0; 2];
    buf.read_looped(&mut first);
    assert_eq!(first, [1, 2]);

    let mut second = [0; 2];
    buf.read_looped(&mut second);
    assert_eq!(second, [3, 1]);
}

#[test]
fn write_stops_at_capacity() {
    let mut buf = LoopBuffer::new(4);
    let written = buf.write(&[1, 2, 3, 4, 5, 6]);
    assert_eq!(written, 4);
    assert_eq!(buf.len(), 4);
    assert_eq!(buf.capacity(), 4);
}

#[test]
fn clear_resets_without_reallocating() {
    let mut buf = LoopBuffer::new(8);
    buf.write(&[1, 2, 3]);
    assert!(!buf.is_empty());

    buf.clear();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
    assert_eq!(buf.capacity(), 8);

    buf.write(&[9, 9]);
    let mut out = [0; 2];
    buf.read_looped(&mut out);
    assert_eq!(out, [9, 9]);
}

#[test]
fn read_looped_on_empty_buffer_yields_silence() {
    let mut buf = LoopBuffer::new(8);
    let mut out = [42; 4];
    buf.read_looped(&mut out);
    assert_eq!(out, [0, 0, 0, 0]);
}
