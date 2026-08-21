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
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        let sm = LoopStateMachine::new();
        assert_eq!(sm.state(), LoopState::Idle);
    }

    #[test]
    fn press_cycles_through_full_sequence() {
        let mut sm = LoopStateMachine::new();

        sm.press();
        assert_eq!(sm.state(), LoopState::Recording);

        sm.press();
        assert_eq!(sm.state(), LoopState::Looping);

        sm.press();
        assert_eq!(sm.state(), LoopState::Stopped);

        sm.press();
        assert_eq!(sm.state(), LoopState::Looping);

        sm.press();
        assert_eq!(sm.state(), LoopState::Stopped);
    }

    #[test]
    fn clear_from_idle_stays_idle() {
        let mut sm = LoopStateMachine::new();
        sm.clear();
        assert_eq!(sm.state(), LoopState::Idle);
    }

    #[test]
    fn clear_from_recording_returns_to_idle() {
        let mut sm = LoopStateMachine::new();
        sm.press();
        assert_eq!(sm.state(), LoopState::Recording);
        sm.clear();
        assert_eq!(sm.state(), LoopState::Idle);
    }

    #[test]
    fn clear_from_looping_returns_to_idle() {
        let mut sm = LoopStateMachine::new();
        sm.press();
        sm.press();
        assert_eq!(sm.state(), LoopState::Looping);
        sm.clear();
        assert_eq!(sm.state(), LoopState::Idle);
    }

    #[test]
    fn clear_from_stopped_returns_to_idle() {
        let mut sm = LoopStateMachine::new();
        sm.press();
        sm.press();
        sm.press();
        assert_eq!(sm.state(), LoopState::Stopped);
        sm.clear();
        assert_eq!(sm.state(), LoopState::Idle);
    }

    #[test]
    fn can_record_again_after_clear() {
        let mut sm = LoopStateMachine::new();
        sm.press();
        sm.press();
        sm.clear();
        sm.press();
        assert_eq!(sm.state(), LoopState::Recording);
    }
}
