//! Test recorder for capturing user interactions into test steps.
//!
//! Converts raw input events into a replayable test sequence.

use crate::testing::runner::TestRunner;
use crate::testing::step::{ClickTarget, TestStep};
use crate::{InputEvent, Key, Modifiers, MouseButton, Point};

/// Records input events into test steps.
pub struct TestRecorder {
    recording: bool,
    steps: Vec<TestStep>,
    cursor: Point,
    pending_text: String,
}

impl Default for TestRecorder {
    fn default() -> Self {
        Self {
            recording: false,
            steps: Vec::new(),
            cursor: Point::ZERO,
            pending_text: String::new(),
        }
    }
}

impl TestRecorder {
    /// Create a new recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start recording events, clearing existing steps.
    pub fn start(&mut self) {
        self.recording = true;
        self.steps.clear();
        self.pending_text.clear();
    }

    /// Stop recording events.
    pub fn stop(&mut self) {
        self.flush_text();
        self.recording = false;
    }

    /// Check whether recording is active.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Access recorded steps.
    pub fn steps(&self) -> &[TestStep] {
        &self.steps
    }

    /// Convert recorded steps into a test runner.
    pub fn into_runner(mut self, name: impl Into<String>) -> TestRunner {
        self.flush_text();
        TestRunner::new(name, self.steps)
    }

    /// Record a single input event.
    pub fn record_event(&mut self, event: &InputEvent) {
        if !self.recording {
            return;
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                self.cursor = Point::new(*x, *y);
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                self.flush_text();
                self.cursor = Point::new(*x, *y);
                self.steps.push(TestStep::Click {
                    target: ClickTarget::Position(self.cursor),
                    button: *button,
                });
            }
            InputEvent::MouseUp { x, y, .. } => {
                self.cursor = Point::new(*x, *y);
            }
            InputEvent::Scroll { dx, dy } => {
                self.flush_text();
                self.steps.push(TestStep::Scroll {
                    target: ClickTarget::Position(self.cursor),
                    dx: *dx,
                    dy: *dy,
                });
            }
            InputEvent::KeyDown { key, modifiers } => {
                self.record_key(key, modifiers);
            }
            InputEvent::KeyUp { .. } => {}
        }
    }

    fn record_key(&mut self, key: &Key, modifiers: &Modifiers) {
        if let Key::Character(ch) = key {
            if !modifiers.ctrl && !modifiers.alt && !modifiers.meta {
                self.pending_text.push_str(ch);
                return;
            }
        }

        self.flush_text();
        self.steps.push(TestStep::KeyPress {
            key: key.clone(),
            modifiers: *modifiers,
        });
    }

    fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }

        let text = std::mem::take(&mut self.pending_text);
        self.steps.push(TestStep::Type {
            text,
            delay_per_char: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NamedKey;

    #[test]
    fn test_records_click_and_scroll() {
        let mut recorder = TestRecorder::new();
        recorder.start();

        recorder.record_event(&InputEvent::MouseDown { button: MouseButton::Left, x: 120.0, y: 80.0, modifiers: Modifiers::default() });
        recorder.record_event(&InputEvent::Scroll { dx: 0.0, dy: 40.0 });
        recorder.stop();

        let steps = recorder.steps();
        assert_eq!(steps.len(), 2);

        match &steps[0] {
            TestStep::Click {
                target: ClickTarget::Position(point),
                button,
            } => {
                assert_eq!(*button, MouseButton::Left);
                assert_eq!(point.x, 120.0);
                assert_eq!(point.y, 80.0);
            }
            _ => panic!("expected click step"),
        }

        match &steps[1] {
            TestStep::Scroll {
                target: ClickTarget::Position(point),
                dx,
                dy,
            } => {
                assert_eq!(point.x, 120.0);
                assert_eq!(point.y, 80.0);
                assert_eq!(*dx, 0.0);
                assert_eq!(*dy, 40.0);
            }
            _ => panic!("expected scroll step"),
        }
    }

    #[test]
    fn test_records_text_and_keypress() {
        let mut recorder = TestRecorder::new();
        recorder.start();

        recorder.record_event(&InputEvent::KeyDown {
            key: Key::Character("h".to_string()),
            modifiers: Modifiers::default(),
        });
        recorder.record_event(&InputEvent::KeyDown {
            key: Key::Character("i".to_string()),
            modifiers: Modifiers::default(),
        });
        recorder.record_event(&InputEvent::KeyDown {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        });
        recorder.stop();

        let steps = recorder.steps();
        assert_eq!(steps.len(), 2);

        match &steps[0] {
            TestStep::Type { text, .. } => {
                assert_eq!(text, "hi");
            }
            _ => panic!("expected type step"),
        }

        match &steps[1] {
            TestStep::KeyPress { key, .. } => {
                if let Key::Named(NamedKey::Enter) = key {
                    // ok
                } else {
                    panic!("expected enter keypress");
                }
            }
            _ => panic!("expected keypress step"),
        }
    }
}
