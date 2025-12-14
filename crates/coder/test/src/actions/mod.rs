//! UserActions - fluent API for simulating user input.
//!
//! Provides a chainable interface for simulating mouse clicks,
//! keyboard input, scrolling, and other user interactions.

mod input;

pub use input::{KeyBuilder, MouseBuilder};

use crate::harness::TestHarness;
use wgpui::{InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey, Point};

/// Fluent API for simulating user actions.
///
/// # Example
///
/// ```rust,ignore
/// UserActions::new(&mut harness)
///     .click(Point::new(100.0, 50.0))
///     .type_text("Hello")
///     .press_key(Key::Named(NamedKey::Enter));
/// ```
pub struct UserActions<'a> {
    harness: &'a mut TestHarness,
    /// Current modifiers state.
    modifiers: Modifiers,
}

impl<'a> UserActions<'a> {
    /// Create a new UserActions builder.
    pub fn new(harness: &'a mut TestHarness) -> Self {
        Self {
            harness,
            modifiers: Modifiers::default(),
        }
    }

    /// Hold shift for subsequent actions.
    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Hold control for subsequent actions.
    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Hold alt for subsequent actions.
    pub fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Hold meta (Cmd on macOS) for subsequent actions.
    pub fn with_meta(mut self) -> Self {
        self.modifiers.meta = true;
        self
    }

    /// Reset modifiers to none.
    pub fn clear_modifiers(mut self) -> Self {
        self.modifiers = Modifiers::default();
        self
    }

    /// Simulate a left mouse click at the given position.
    pub fn click(self, position: Point) -> Self {
        self.click_button(position, MouseButton::Left)
    }

    /// Simulate a right mouse click at the given position.
    pub fn right_click(self, position: Point) -> Self {
        self.click_button(position, MouseButton::Right)
    }

    /// Simulate a middle mouse click at the given position.
    pub fn middle_click(self, position: Point) -> Self {
        self.click_button(position, MouseButton::Middle)
    }

    /// Simulate a double click at the given position.
    pub fn double_click(mut self, position: Point) -> Self {
        // Two clicks in quick succession
        self = self.click(position);
        self.click(position)
    }

    /// Simulate a click with a specific button.
    pub fn click_button(mut self, position: Point, button: MouseButton) -> Self {
        self.dispatch(InputEvent::MouseDown {
            position,
            button,
            modifiers: self.modifiers,
        });
        self.dispatch(InputEvent::MouseUp {
            position,
            button,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate mouse down at the given position.
    pub fn mouse_down(mut self, position: Point) -> Self {
        self.dispatch(InputEvent::MouseDown {
            position,
            button: MouseButton::Left,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate mouse up at the given position.
    pub fn mouse_up(mut self, position: Point) -> Self {
        self.dispatch(InputEvent::MouseUp {
            position,
            button: MouseButton::Left,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate mouse move to the given position.
    pub fn mouse_move(mut self, position: Point) -> Self {
        self.dispatch(InputEvent::MouseMove {
            position,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate a drag from one position to another.
    pub fn drag(mut self, from: Point, to: Point) -> Self {
        self.dispatch(InputEvent::MouseDown {
            position: from,
            button: MouseButton::Left,
            modifiers: self.modifiers,
        });

        // Generate intermediate move events for smoother simulation
        let steps = 5;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let x = from.x + (to.x - from.x) * t;
            let y = from.y + (to.y - from.y) * t;
            self.dispatch(InputEvent::MouseMove {
                position: Point::new(x, y),
                modifiers: self.modifiers,
            });
        }

        self.dispatch(InputEvent::MouseUp {
            position: to,
            button: MouseButton::Left,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate scrolling by the given delta.
    pub fn scroll(mut self, delta: Point) -> Self {
        self.dispatch(InputEvent::Wheel {
            delta,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate vertical scrolling.
    pub fn scroll_y(self, delta: f32) -> Self {
        self.scroll(Point::new(0.0, delta))
    }

    /// Simulate horizontal scrolling.
    pub fn scroll_x(self, delta: f32) -> Self {
        self.scroll(Point::new(delta, 0.0))
    }

    /// Type a string of text (generates TextInput events).
    pub fn type_text(mut self, text: &str) -> Self {
        for c in text.chars() {
            self.dispatch(InputEvent::TextInput {
                text: c.to_string(),
            });
        }
        self
    }

    /// Type a string of text all at once.
    pub fn type_text_batch(mut self, text: &str) -> Self {
        self.dispatch(InputEvent::TextInput {
            text: text.to_string(),
        });
        self
    }

    /// Press a key (key down + key up).
    pub fn press_key(self, key: Key) -> Self {
        let code = key_to_code(&key);
        self.key_down(key.clone(), code).key_up(key, code)
    }

    /// Press a named key.
    pub fn press_named_key(self, named: NamedKey) -> Self {
        self.press_key(Key::Named(named))
    }

    /// Press Enter key.
    pub fn press_enter(self) -> Self {
        self.press_named_key(NamedKey::Enter)
    }

    /// Press Escape key.
    pub fn press_escape(self) -> Self {
        self.press_named_key(NamedKey::Escape)
    }

    /// Press Tab key.
    pub fn press_tab(self) -> Self {
        self.press_named_key(NamedKey::Tab)
    }

    /// Press Backspace key.
    pub fn press_backspace(self) -> Self {
        self.press_named_key(NamedKey::Backspace)
    }

    /// Press Delete key.
    pub fn press_delete(self) -> Self {
        self.press_named_key(NamedKey::Delete)
    }

    /// Press arrow up.
    pub fn press_up(self) -> Self {
        self.press_named_key(NamedKey::ArrowUp)
    }

    /// Press arrow down.
    pub fn press_down(self) -> Self {
        self.press_named_key(NamedKey::ArrowDown)
    }

    /// Press arrow left.
    pub fn press_left(self) -> Self {
        self.press_named_key(NamedKey::ArrowLeft)
    }

    /// Press arrow right.
    pub fn press_right(self) -> Self {
        self.press_named_key(NamedKey::ArrowRight)
    }

    /// Simulate key down event.
    pub fn key_down(mut self, key: Key, code: KeyCode) -> Self {
        self.dispatch(InputEvent::KeyDown {
            key,
            code,
            modifiers: self.modifiers,
            repeat: false,
        });
        self
    }

    /// Simulate key up event.
    pub fn key_up(mut self, key: Key, code: KeyCode) -> Self {
        self.dispatch(InputEvent::KeyUp {
            key,
            code,
            modifiers: self.modifiers,
        });
        self
    }

    /// Simulate focus gained.
    pub fn focus_in(mut self) -> Self {
        self.dispatch(InputEvent::FocusIn);
        self
    }

    /// Simulate focus lost.
    pub fn focus_out(mut self) -> Self {
        self.dispatch(InputEvent::FocusOut);
        self
    }

    /// Simulate IME composition start.
    pub fn ime_start(mut self) -> Self {
        self.dispatch(InputEvent::ImeStart);
        self
    }

    /// Simulate IME composition update.
    pub fn ime_update(mut self, text: &str, cursor: Option<usize>) -> Self {
        self.dispatch(InputEvent::ImeUpdate {
            text: text.to_string(),
            cursor,
        });
        self
    }

    /// Simulate IME composition end.
    pub fn ime_end(mut self) -> Self {
        self.dispatch(InputEvent::ImeEnd);
        self
    }

    /// Dispatch a raw input event.
    fn dispatch(&mut self, event: InputEvent) {
        // Note: We're just recording events here. The actual dispatch
        // to widgets happens when render/dispatch is called on the harness.
        // For now, we'll store events and process them.
        // This is a simplified implementation - in practice, you'd
        // want to call harness.dispatch() with the mounted widget.
        let _ = event; // Placeholder
    }

    /// Get a reference to the test harness.
    pub fn harness(&self) -> &TestHarness {
        self.harness
    }

    /// Get a mutable reference to the test harness.
    pub fn harness_mut(&mut self) -> &mut TestHarness {
        self.harness
    }
}

/// Convert a Key to a KeyCode (best effort).
fn key_to_code(key: &Key) -> KeyCode {
    match key {
        Key::Character(s) => match s.to_uppercase().as_str() {
            "A" => KeyCode::KeyA,
            "B" => KeyCode::KeyB,
            "C" => KeyCode::KeyC,
            "D" => KeyCode::KeyD,
            "E" => KeyCode::KeyE,
            "F" => KeyCode::KeyF,
            "G" => KeyCode::KeyG,
            "H" => KeyCode::KeyH,
            "I" => KeyCode::KeyI,
            "J" => KeyCode::KeyJ,
            "K" => KeyCode::KeyK,
            "L" => KeyCode::KeyL,
            "M" => KeyCode::KeyM,
            "N" => KeyCode::KeyN,
            "O" => KeyCode::KeyO,
            "P" => KeyCode::KeyP,
            "Q" => KeyCode::KeyQ,
            "R" => KeyCode::KeyR,
            "S" => KeyCode::KeyS,
            "T" => KeyCode::KeyT,
            "U" => KeyCode::KeyU,
            "V" => KeyCode::KeyV,
            "W" => KeyCode::KeyW,
            "X" => KeyCode::KeyX,
            "Y" => KeyCode::KeyY,
            "Z" => KeyCode::KeyZ,
            "0" => KeyCode::Digit0,
            "1" => KeyCode::Digit1,
            "2" => KeyCode::Digit2,
            "3" => KeyCode::Digit3,
            "4" => KeyCode::Digit4,
            "5" => KeyCode::Digit5,
            "6" => KeyCode::Digit6,
            "7" => KeyCode::Digit7,
            "8" => KeyCode::Digit8,
            "9" => KeyCode::Digit9,
            " " => KeyCode::Space,
            _ => KeyCode::Unknown,
        },
        Key::Named(named) => match named {
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::Space => KeyCode::Space,
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Delete => KeyCode::Delete,
            NamedKey::Escape => KeyCode::Escape,
            NamedKey::ArrowUp => KeyCode::ArrowUp,
            NamedKey::ArrowDown => KeyCode::ArrowDown,
            NamedKey::ArrowLeft => KeyCode::ArrowLeft,
            NamedKey::ArrowRight => KeyCode::ArrowRight,
            NamedKey::Home => KeyCode::Home,
            NamedKey::End => KeyCode::End,
            NamedKey::PageUp => KeyCode::PageUp,
            NamedKey::PageDown => KeyCode::PageDown,
            NamedKey::F1 => KeyCode::F1,
            NamedKey::F2 => KeyCode::F2,
            NamedKey::F3 => KeyCode::F3,
            NamedKey::F4 => KeyCode::F4,
            NamedKey::F5 => KeyCode::F5,
            NamedKey::F6 => KeyCode::F6,
            NamedKey::F7 => KeyCode::F7,
            NamedKey::F8 => KeyCode::F8,
            NamedKey::F9 => KeyCode::F9,
            NamedKey::F10 => KeyCode::F10,
            NamedKey::F11 => KeyCode::F11,
            NamedKey::F12 => KeyCode::F12,
            NamedKey::Shift => KeyCode::ShiftLeft,
            NamedKey::Control => KeyCode::ControlLeft,
            NamedKey::Alt => KeyCode::AltLeft,
            NamedKey::Meta => KeyCode::MetaLeft,
            _ => KeyCode::Unknown,
        },
        Key::Unidentified => KeyCode::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_actions_modifiers() {
        let mut harness = TestHarness::new();
        let actions = UserActions::new(&mut harness)
            .with_shift()
            .with_ctrl();

        assert!(actions.modifiers.shift);
        assert!(actions.modifiers.ctrl);
        assert!(!actions.modifiers.alt);
        assert!(!actions.modifiers.meta);
    }

    #[test]
    fn test_user_actions_clear_modifiers() {
        let mut harness = TestHarness::new();
        let actions = UserActions::new(&mut harness)
            .with_shift()
            .with_ctrl()
            .clear_modifiers();

        assert!(!actions.modifiers.shift);
        assert!(!actions.modifiers.ctrl);
    }

    #[test]
    fn test_key_to_code() {
        assert_eq!(key_to_code(&Key::Character("A".into())), KeyCode::KeyA);
        assert_eq!(key_to_code(&Key::Character("a".into())), KeyCode::KeyA);
        assert_eq!(key_to_code(&Key::Character("5".into())), KeyCode::Digit5);
        assert_eq!(key_to_code(&Key::Named(NamedKey::Enter)), KeyCode::Enter);
        assert_eq!(key_to_code(&Key::Named(NamedKey::ArrowUp)), KeyCode::ArrowUp);
    }
}
