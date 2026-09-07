/// Mimics a classic single-footswitch looper pedal (e.g. TC Electronic
/// Ditto): one control cycles Idle -> Recording -> Looping -> Stopped ->
/// Looping -> ..., and a long-press clears from any state back to Idle.
/// Overdub sits off that cycle on a control of its own, so the press
/// cycle keeps behaving exactly as it always has.
///
/// The discriminants cross the thread boundary as a `u8` - see
/// `SharedControl::load_state` - so their order is not free to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Recording,
    Looping,
    Stopped,
    Overdubbing,
}

pub struct LoopStateMachine {
    state: LoopState,
}

impl LoopStateMachine {
    pub fn new() -> Self {
        Self {
            state: LoopState::Idle,
        }
    }

    pub fn state(&self) -> LoopState {
        self.state
    }

    /// Short press: advances the cycle. Both playing states stop, so the
    /// main control always means "stop" while something is playing.
    pub fn press(&mut self) {
        self.state = match self.state {
            LoopState::Idle => LoopState::Recording,
            LoopState::Recording => LoopState::Looping,
            LoopState::Looping | LoopState::Overdubbing => LoopState::Stopped,
            LoopState::Stopped => LoopState::Looping,
        };
    }

    /// The overdub control: opens a new layer over the playing loop, or
    /// closes the one in progress. Nothing to do from any other state -
    /// there's no loop to overdub onto.
    pub fn toggle_overdub(&mut self) {
        self.state = match self.state {
            LoopState::Looping => LoopState::Overdubbing,
            LoopState::Overdubbing => LoopState::Looping,
            other => other,
        };
    }

    /// Long-press (~2s hold): clears the loop from any state, back to Idle.
    pub fn clear(&mut self) {
        self.state = LoopState::Idle;
    }
}

impl Default for LoopStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "state_machine_tests.rs"]
mod tests;
