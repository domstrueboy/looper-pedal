use std::time::{Duration, Instant};

const LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(2000);

/// Turns a raw "is the button down right now" signal into short-press vs
/// long-press-clear events, mimicking a real pedal footswitch. Long-press
/// fires the moment the hold crosses the threshold, while still held; a
/// short press only fires on release, once confirmed not to be a long
/// press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    None,
    ShortPress,
    LongPressClear,
}

pub struct InputHandler {
    pressed_at: Option<Instant>,
    long_press_fired: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            pressed_at: None,
            long_press_fired: false,
        }
    }

    /// True once a hold has crossed the long-press threshold and is still
    /// being held (i.e. after the frame that returned `LongPressClear`,
    /// until release) - for UI feedback confirming the clear that just
    /// fired, distinct from the one-shot `LongPressClear` event itself.
    pub fn is_long_press_active(&self) -> bool {
        self.pressed_at.is_some() && self.long_press_fired
    }

    /// Call once per frame with whether the button/key is currently held
    /// down and the current time.
    pub fn update(&mut self, is_down: bool, now: Instant) -> InputEvent {
        if is_down {
            match self.pressed_at {
                None => {
                    self.pressed_at = Some(now);
                    self.long_press_fired = false;
                    InputEvent::None
                }
                Some(pressed_at) => {
                    if !self.long_press_fired && now.duration_since(pressed_at) >= LONG_PRESS_THRESHOLD
                    {
                        self.long_press_fired = true;
                        InputEvent::LongPressClear
                    } else {
                        InputEvent::None
                    }
                }
            }
        } else if self.pressed_at.take().is_some() && !self.long_press_fired {
            InputEvent::ShortPress
        } else {
            InputEvent::None
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
