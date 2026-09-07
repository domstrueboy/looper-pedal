use std::time::{Duration, Instant};

const LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(2000);

/// Turns "is it down right now" into short-press vs long-press-clear, like
/// a real footswitch: long-press fires the moment the threshold is crossed,
/// while still held; a short press fires on release, once it's known not to
/// be a long one.
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

    /// True from the frame `LongPressClear` fired until release - for UI
    /// confirming the clear, distinct from the one-shot event itself.
    pub fn is_long_press_active(&self) -> bool {
        self.pressed_at.is_some() && self.long_press_fired
    }

    /// Call once per frame.
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
