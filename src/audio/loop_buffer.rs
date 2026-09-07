/// Pre-allocated buffer for one recorded loop. Recording appends up to
/// `capacity`; playback wraps at the recorded length, not the capacity, so
/// the loop repeats seamlessly however much was recorded. Nothing allocates
/// outside `new`, so it's safe in a real-time callback.
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

    /// Appends samples, stopping at capacity. Returns how many were written.
    pub fn write(&mut self, input: &[i32]) -> usize {
        let space = self.capacity() - self.len;
        let to_write = input.len().min(space);
        self.samples[self.len..self.len + to_write].copy_from_slice(&input[..to_write]);
        self.len += to_write;
        to_write
    }

    /// Fills `out` from the current playback position, wrapping at the
    /// recorded length and advancing across calls. Silence if empty.
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

    /// Drops the loop and resets playback. Does not reallocate.
    pub fn clear(&mut self) {
        self.len = 0;
        self.play_pos = 0;
    }
}

#[cfg(test)]
#[path = "loop_buffer_tests.rs"]
mod tests;
