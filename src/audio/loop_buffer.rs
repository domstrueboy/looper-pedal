/// A pre-allocated buffer for a single recorded loop. Recording appends
/// samples sequentially up to `capacity`; playback reads them back wrapping
/// around at the recorded length (not the capacity), so the loop repeats
/// seamlessly regardless of how many samples were actually recorded.
///
/// No allocation happens outside of `new` - safe to use from a real-time
/// audio callback.
pub struct LoopBuffer {
    samples: Vec<i32>,
    len: usize,
    play_pos: usize,
}

impl LoopBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![0; capacity],
            len: 0,
            play_pos: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.samples.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends samples, stopping at capacity. Returns how many were
    /// actually written (less than `input.len()` once capacity is hit).
    pub fn write(&mut self, input: &[i32]) -> usize {
        let space = self.capacity() - self.len;
        let to_write = input.len().min(space);
        self.samples[self.len..self.len + to_write].copy_from_slice(&input[..to_write]);
        self.len += to_write;
        to_write
    }

    /// Fills `out` with samples starting from the current playback
    /// position, wrapping around at the recorded length. Advances the
    /// playback position across calls. No-op if nothing has been recorded.
    pub fn read_looped(&mut self, out: &mut [i32]) {
        if self.len == 0 {
            out.fill(0);
            return;
        }

        for sample in out.iter_mut() {
            *sample = self.samples[self.play_pos];
            self.play_pos = (self.play_pos + 1) % self.len;
        }
    }

    /// Drops the recorded loop and resets playback, ready for a new
    /// recording. Does not reallocate.
    pub fn clear(&mut self) {
        self.len = 0;
        self.play_pos = 0;
    }
}

#[cfg(test)]
mod tests {
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
}
