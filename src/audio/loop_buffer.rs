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

    pub fn play_pos(&self) -> usize {
        self.play_pos
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
#[path = "loop_buffer_tests.rs"]
mod tests;
