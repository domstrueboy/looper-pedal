/// Most layers that can be stacked, the first recording included. Every
/// layer is a full `capacity`-sized buffer allocated up front - nothing
/// allocates on the audio thread - so this is a straight memory
/// multiplier: ~11.5 MB per layer at 48 kHz with a 60s capacity.
pub const MAX_LAYERS: usize = 4;

/// One recorded layer, indexed by absolute position in the loop.
///
/// An overdub starts wherever playback happens to be and can be stopped
/// early, so a layer covers only the window it was recorded over:
/// `written` samples from `start` onward. Outside that window the buffer
/// still holds an earlier take's samples and is simply never read - which
/// is what lets a layer be reused without memsetting megabytes inside the
/// audio callback.
struct Layer {
    samples: Vec<i32>,
    start: usize,
    written: usize,
}

impl Layer {
    fn new(capacity: usize) -> Self {
        Self {
            samples: vec![0; capacity],
            start: 0,
            written: 0,
        }
    }

    /// How far `pos` is past the point this layer started recording,
    /// wrapping with the loop.
    fn offset_from_start(&self, pos: usize, len: usize) -> usize {
        (pos + len - self.start) % len
    }

    /// What this layer contributes at loop position `pos` - zero outside
    /// the window it was recorded over.
    fn sample_at(&self, pos: usize, len: usize) -> i32 {
        if self.offset_from_start(pos, len) < self.written {
            self.samples[pos]
        } else {
            0
        }
    }
}

/// A stack of aligned loop layers: the first recording fixes the loop
/// length, and each overdub adds another layer on top of it, playing back
/// as their sum. Layers are recorded and dropped independently, which is
/// the same data model multitrack needs later.
///
/// Nothing allocates outside `new`, so it's safe in a real-time callback.
pub struct LoopStack {
    layers: Vec<Layer>,
    /// How many layers hold a finished take.
    count: usize,
    /// Loop length in samples, fixed when the first layer stops recording;
    /// 0 while empty.
    len: usize,
    play_pos: usize,
    /// The layer being written to right now, if any. It sits just above
    /// `count` and is only counted once the take is finished.
    recording: Option<usize>,
}

impl LoopStack {
    pub fn new(capacity: usize) -> Self {
        Self {
            layers: (0..MAX_LAYERS).map(|_| Layer::new(capacity)).collect(),
            count: 0,
            len: 0,
            play_pos: 0,
            recording: None,
        }
    }

    pub fn capacity(&self) -> usize {
        self.layers[0].samples.len()
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

    pub fn layer_count(&self) -> usize {
        self.count
    }

    pub fn is_full(&self) -> bool {
        self.count >= MAX_LAYERS
    }

    /// Starts the first layer, dropping whatever was there. The loop
    /// length isn't known until `finish_first_layer`, so this take simply
    /// appends from zero.
    pub fn begin_first_layer(&mut self) {
        self.clear();
        self.layers[0].start = 0;
        self.layers[0].written = 0;
        self.recording = Some(0);
    }

    /// Appends to the first layer, stopping at capacity. Returns how many
    /// samples were written.
    pub fn record_first_layer(&mut self, input: &[i32]) -> usize {
        let Some(index) = self.recording else {
            return 0;
        };
        let capacity = self.capacity();
        let layer = &mut self.layers[index];
        let n = input.len().min(capacity - layer.written);
        layer.samples[layer.written..layer.written + n].copy_from_slice(&input[..n]);
        layer.written += n;
        n
    }

    /// Fixes the loop length at whatever was recorded and rewinds to the
    /// top. A take with nothing in it leaves the stack empty.
    pub fn finish_first_layer(&mut self) {
        let Some(index) = self.recording.take() else {
            return;
        };
        self.len = self.layers[index].written;
        self.play_pos = 0;
        self.count = usize::from(self.len > 0);
    }

    /// Starts a layer aligned to the current playback position. False if
    /// there's no loop to overdub onto, or no layer left.
    pub fn begin_overdub(&mut self) -> bool {
        if self.is_empty() || self.is_full() {
            return false;
        }
        let index = self.count;
        self.layers[index].start = self.play_pos;
        self.layers[index].written = 0;
        self.recording = Some(index);
        true
    }

    /// Keeps the take just recorded. One with nothing in it is dropped
    /// rather than adding a silent layer.
    pub fn finish_overdub(&mut self) {
        let Some(index) = self.recording.take() else {
            return;
        };
        if self.layers[index].written > 0 {
            self.count = index + 1;
        }
    }

    /// Drops the newest finished layer, leaving the loop length and
    /// playback position alone so the layers underneath keep playing.
    pub fn remove_last_layer(&mut self) {
        self.count = self.count.saturating_sub(1);
        if self.count == 0 {
            self.clear();
        }
    }

    /// Drops every layer and resets playback. Does not reallocate - the
    /// buffers keep their stale contents, which the recorded-window logic
    /// makes unreadable.
    pub fn clear(&mut self) {
        self.count = 0;
        self.len = 0;
        self.play_pos = 0;
        self.recording = None;
    }

    /// One playback pass: mixes every layer into `out`, advancing the
    /// shared playback position. Silence while empty.
    pub fn read_mixed(&mut self, out: &mut [i32]) {
        if self.len == 0 {
            out.fill(0);
            return;
        }
        for sample in out.iter_mut() {
            *sample = self.mix_at(self.play_pos);
            self.play_pos = (self.play_pos + 1) % self.len;
        }
    }

    /// Playback and overdub in one pass. Each sample is mixed into `out`
    /// *before* the matching `input` sample is recorded over it, so the
    /// take being played isn't echoed straight back on top of the
    /// player's own live signal - it comes back from the next pass on.
    /// Recording a second pass over the same layer sums into it rather
    /// than replacing it, so nothing already played is erased.
    pub fn read_mixed_with_overdub(&mut self, out: &mut [i32], input: &[i32]) {
        let len = self.len;
        let Some(index) = self.recording.filter(|_| len > 0) else {
            self.read_mixed(out);
            return;
        };

        for (i, sample) in out.iter_mut().enumerate() {
            let pos = self.play_pos;
            *sample = self.mix_at(pos);

            let recorded = input.get(i).copied().unwrap_or(0);
            let layer = &mut self.layers[index];
            let offset = layer.offset_from_start(pos, len);
            if offset < layer.written {
                layer.samples[pos] = layer.samples[pos].saturating_add(recorded);
            } else {
                layer.samples[pos] = recorded;
                // Walked forward one more sample from `start`; the window
                // is what makes the rest of the buffer unreadable.
                layer.written = offset + 1;
            }

            self.play_pos = (pos + 1) % len;
        }
    }

    /// Every layer's contribution at `pos`, summed. The take in progress
    /// is included - only over the part of it already recorded - so a
    /// second pass hears the first one come back around.
    fn mix_at(&self, pos: usize) -> i32 {
        let active = match self.recording {
            Some(index) => index + 1,
            None => self.count,
        };
        self.layers[..active].iter().fold(0i32, |sum, layer| {
            sum.saturating_add(layer.sample_at(pos, self.len))
        })
    }
}

#[cfg(test)]
#[path = "loop_stack_tests.rs"]
mod tests;
