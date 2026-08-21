/// Mimics a classic single-footswitch looper pedal (e.g. TC Electronic
/// Ditto): one control cycles Idle -> Recording -> Looping -> Stopped ->
/// Looping -> ..., and a long-press clears from any state back to Idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Idle,
    Recording,
    Looping,
    Stopped,
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

    /// Short press: advances the 4-state cycle.
    pub fn press(&mut self) {
        self.state = match self.state {
            LoopState::Idle => LoopState::Recording,
            LoopState::Recording => LoopState::Looping,
            LoopState::Looping => LoopState::Stopped,
            LoopState::Stopped => LoopState::Looping,
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
