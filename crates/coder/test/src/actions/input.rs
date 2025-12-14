//! Input builders for complex input sequences.

use wgpui::{InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey, Point};

/// Builder for constructing mouse events.
pub struct MouseBuilder {
    position: Point,
    button: MouseButton,
    modifiers: Modifiers,
}

impl MouseBuilder {
    /// Create a new mouse builder at the given position.
    pub fn new(position: Point) -> Self {
        Self {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        }
    }

    /// Set the mouse button.
    pub fn button(mut self, button: MouseButton) -> Self {
        self.button = button;
        self
    }

    /// Set modifiers.
    pub fn modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Add shift modifier.
    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Add control modifier.
    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Add alt modifier.
    pub fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Add meta modifier.
    pub fn with_meta(mut self) -> Self {
        self.modifiers.meta = true;
        self
    }

    /// Build a mouse down event.
    pub fn down(self) -> InputEvent {
        InputEvent::MouseDown {
            position: self.position,
            button: self.button,
            modifiers: self.modifiers,
        }
    }

    /// Build a mouse up event.
    pub fn up(self) -> InputEvent {
        InputEvent::MouseUp {
            position: self.position,
            button: self.button,
            modifiers: self.modifiers,
        }
    }

    /// Build a mouse move event.
    pub fn move_to(self) -> InputEvent {
        InputEvent::MouseMove {
            position: self.position,
            modifiers: self.modifiers,
        }
    }

    /// Build a click sequence (down + up).
    pub fn click(self) -> Vec<InputEvent> {
        vec![
            InputEvent::MouseDown {
                position: self.position,
                button: self.button,
                modifiers: self.modifiers,
            },
            InputEvent::MouseUp {
                position: self.position,
                button: self.button,
                modifiers: self.modifiers,
            },
        ]
    }
}

/// Builder for constructing keyboard events.
pub struct KeyBuilder {
    key: Key,
    code: KeyCode,
    modifiers: Modifiers,
    repeat: bool,
}

impl KeyBuilder {
    /// Create a new key builder for a character key.
    pub fn char(c: char) -> Self {
        let key = Key::Character(c.to_string());
        let code = char_to_code(c);
        Self {
            key,
            code,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    /// Create a new key builder for a named key.
    pub fn named(named: NamedKey) -> Self {
        let key = Key::Named(named);
        let code = named_to_code(named);
        Self {
            key,
            code,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    /// Set the key code explicitly.
    pub fn code(mut self, code: KeyCode) -> Self {
        self.code = code;
        self
    }

    /// Set modifiers.
    pub fn modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Add shift modifier.
    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Add control modifier.
    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Add alt modifier.
    pub fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Add meta modifier.
    pub fn with_meta(mut self) -> Self {
        self.modifiers.meta = true;
        self
    }

    /// Mark as a repeat event.
    pub fn repeat(mut self) -> Self {
        self.repeat = true;
        self
    }

    /// Build a key down event.
    pub fn down(self) -> InputEvent {
        InputEvent::KeyDown {
            key: self.key,
            code: self.code,
            modifiers: self.modifiers,
            repeat: self.repeat,
        }
    }

    /// Build a key up event.
    pub fn up(self) -> InputEvent {
        InputEvent::KeyUp {
            key: self.key,
            code: self.code,
            modifiers: self.modifiers,
        }
    }

    /// Build a key press sequence (down + up).
    pub fn press(self) -> Vec<InputEvent> {
        vec![
            InputEvent::KeyDown {
                key: self.key.clone(),
                code: self.code,
                modifiers: self.modifiers,
                repeat: self.repeat,
            },
            InputEvent::KeyUp {
                key: self.key,
                code: self.code,
                modifiers: self.modifiers,
            },
        ]
    }
}

/// Convert a character to a key code.
fn char_to_code(c: char) -> KeyCode {
    match c.to_uppercase().next().unwrap_or(c) {
        'A' => KeyCode::KeyA,
        'B' => KeyCode::KeyB,
        'C' => KeyCode::KeyC,
        'D' => KeyCode::KeyD,
        'E' => KeyCode::KeyE,
        'F' => KeyCode::KeyF,
        'G' => KeyCode::KeyG,
        'H' => KeyCode::KeyH,
        'I' => KeyCode::KeyI,
        'J' => KeyCode::KeyJ,
        'K' => KeyCode::KeyK,
        'L' => KeyCode::KeyL,
        'M' => KeyCode::KeyM,
        'N' => KeyCode::KeyN,
        'O' => KeyCode::KeyO,
        'P' => KeyCode::KeyP,
        'Q' => KeyCode::KeyQ,
        'R' => KeyCode::KeyR,
        'S' => KeyCode::KeyS,
        'T' => KeyCode::KeyT,
        'U' => KeyCode::KeyU,
        'V' => KeyCode::KeyV,
        'W' => KeyCode::KeyW,
        'X' => KeyCode::KeyX,
        'Y' => KeyCode::KeyY,
        'Z' => KeyCode::KeyZ,
        '0' => KeyCode::Digit0,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        '3' => KeyCode::Digit3,
        '4' => KeyCode::Digit4,
        '5' => KeyCode::Digit5,
        '6' => KeyCode::Digit6,
        '7' => KeyCode::Digit7,
        '8' => KeyCode::Digit8,
        '9' => KeyCode::Digit9,
        ' ' => KeyCode::Space,
        '-' => KeyCode::Minus,
        '=' => KeyCode::Equal,
        '[' => KeyCode::BracketLeft,
        ']' => KeyCode::BracketRight,
        '\\' => KeyCode::Backslash,
        ';' => KeyCode::Semicolon,
        '\'' => KeyCode::Quote,
        '`' => KeyCode::Backquote,
        ',' => KeyCode::Comma,
        '.' => KeyCode::Period,
        '/' => KeyCode::Slash,
        _ => KeyCode::Unknown,
    }
}

/// Convert a named key to a key code.
fn named_to_code(named: NamedKey) -> KeyCode {
    match named {
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
        NamedKey::Insert => KeyCode::Insert,
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
        NamedKey::CapsLock => KeyCode::CapsLock,
        NamedKey::NumLock => KeyCode::NumLock,
        NamedKey::ScrollLock => KeyCode::ScrollLock,
        NamedKey::PrintScreen => KeyCode::PrintScreen,
        NamedKey::Pause => KeyCode::Pause,
        NamedKey::ContextMenu => KeyCode::ContextMenu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_builder() {
        let pos = Point::new(100.0, 50.0);
        let builder = MouseBuilder::new(pos).with_shift();

        let down = builder.down();
        match down {
            InputEvent::MouseDown {
                position,
                modifiers,
                ..
            } => {
                assert_eq!(position, pos);
                assert!(modifiers.shift);
            }
            _ => panic!("Expected MouseDown"),
        }
    }

    #[test]
    fn test_mouse_click_sequence() {
        let pos = Point::new(0.0, 0.0);
        let events = MouseBuilder::new(pos).click();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], InputEvent::MouseDown { .. }));
        assert!(matches!(events[1], InputEvent::MouseUp { .. }));
    }

    #[test]
    fn test_key_builder_char() {
        let builder = KeyBuilder::char('a').with_ctrl();
        let down = builder.down();

        match down {
            InputEvent::KeyDown {
                key,
                code,
                modifiers,
                ..
            } => {
                assert_eq!(key, Key::Character("a".into()));
                assert_eq!(code, KeyCode::KeyA);
                assert!(modifiers.ctrl);
            }
            _ => panic!("Expected KeyDown"),
        }
    }

    #[test]
    fn test_key_builder_named() {
        let events = KeyBuilder::named(NamedKey::Enter).press();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], InputEvent::KeyDown { .. }));
        assert!(matches!(events[1], InputEvent::KeyUp { .. }));
    }

    #[test]
    fn test_char_to_code() {
        assert_eq!(char_to_code('a'), KeyCode::KeyA);
        assert_eq!(char_to_code('A'), KeyCode::KeyA);
        assert_eq!(char_to_code('5'), KeyCode::Digit5);
        assert_eq!(char_to_code(' '), KeyCode::Space);
    }
}
