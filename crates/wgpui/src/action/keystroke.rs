//! Keystroke parsing and matching.

use crate::{Key, Modifiers, NamedKey};
use std::fmt;

/// A single keystroke with modifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    /// The key pressed.
    pub key: Key,
    /// Modifier keys held during the keystroke.
    pub modifiers: Modifiers,
}

impl Keystroke {
    /// Create a new keystroke.
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Parse from string like "cmd-shift-s", "ctrl-c", "escape", "enter".
    ///
    /// # Supported Modifiers
    /// - `cmd`, `meta`, `super` - Meta/Command key
    /// - `ctrl`, `control` - Control key
    /// - `alt`, `opt`, `option` - Alt/Option key
    /// - `shift` - Shift key
    ///
    /// # Supported Keys
    /// - Single characters: `a`, `b`, `1`, etc.
    /// - Named keys: `enter`, `escape`, `tab`, `backspace`, `delete`,
    ///   `up`, `down`, `left`, `right`, `home`, `end`
    ///
    /// # Example
    /// ```ignore
    /// let ks = Keystroke::parse("cmd-shift-s").unwrap();
    /// assert!(ks.modifiers.meta);
    /// assert!(ks.modifiers.shift);
    /// ```
    pub fn parse(input: &str) -> Result<Self, KeystrokeParseError> {
        let parts: Vec<&str> = input.split('-').collect();
        let mut modifiers = Modifiers::default();
        let mut key_str: Option<String> = None;

        for part in parts {
            let lower = part.to_lowercase();
            match lower.as_str() {
                "cmd" | "meta" | "super" => modifiers.meta = true,
                "ctrl" | "control" => modifiers.ctrl = true,
                "alt" | "opt" | "option" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                _ => {
                    if key_str.is_some() {
                        return Err(KeystrokeParseError::MultipleKeys);
                    }
                    key_str = Some(lower);
                }
            }
        }

        let key = match key_str.as_deref() {
            Some("enter") | Some("return") => Key::Named(NamedKey::Enter),
            Some("escape") | Some("esc") => Key::Named(NamedKey::Escape),
            Some("tab") => Key::Named(NamedKey::Tab),
            Some("backspace") => Key::Named(NamedKey::Backspace),
            Some("delete") | Some("del") => Key::Named(NamedKey::Delete),
            Some("up") | Some("arrowup") => Key::Named(NamedKey::ArrowUp),
            Some("down") | Some("arrowdown") => Key::Named(NamedKey::ArrowDown),
            Some("left") | Some("arrowleft") => Key::Named(NamedKey::ArrowLeft),
            Some("right") | Some("arrowright") => Key::Named(NamedKey::ArrowRight),
            Some("home") => Key::Named(NamedKey::Home),
            Some("end") => Key::Named(NamedKey::End),
            Some(s) => Key::Character(s.to_string()),
            None => return Err(KeystrokeParseError::NoKey),
        };

        Ok(Self { key, modifiers })
    }

    /// Check if this keystroke matches an input event.
    pub fn matches(&self, key: &Key, modifiers: &Modifiers) -> KeystrokeMatch {
        // Check modifiers first
        if self.modifiers.ctrl != modifiers.ctrl
            || self.modifiers.alt != modifiers.alt
            || self.modifiers.meta != modifiers.meta
            || self.modifiers.shift != modifiers.shift
        {
            return KeystrokeMatch::None;
        }

        // Check key
        match (&self.key, key) {
            (Key::Named(a), Key::Named(b)) if a == b => KeystrokeMatch::Matched,
            (Key::Character(a), Key::Character(b)) if a.eq_ignore_ascii_case(b) => {
                KeystrokeMatch::Matched
            }
            _ => KeystrokeMatch::None,
        }
    }
}

/// Result of matching a keystroke against input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeystrokeMatch {
    /// The keystroke fully matches.
    Matched,
    /// The keystroke is a prefix of a multi-key sequence (future use).
    Pending,
    /// No match.
    None,
}

/// Error parsing a keystroke string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystrokeParseError {
    /// No key specified in the keystroke string.
    NoKey,
    /// Multiple non-modifier keys specified.
    MultipleKeys,
    /// Invalid key name.
    InvalidKey(String),
}

impl fmt::Display for KeystrokeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeystrokeParseError::NoKey => write!(f, "no key specified in keystroke"),
            KeystrokeParseError::MultipleKeys => {
                write!(f, "multiple non-modifier keys in keystroke")
            }
            KeystrokeParseError::InvalidKey(k) => write!(f, "invalid key: {}", k),
        }
    }
}

impl std::error::Error for KeystrokeParseError {}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.meta {
            parts.push("cmd");
        }
        if self.modifiers.ctrl {
            parts.push("ctrl");
        }
        if self.modifiers.alt {
            parts.push("alt");
        }
        if self.modifiers.shift {
            parts.push("shift");
        }

        let key_str = match &self.key {
            Key::Named(NamedKey::Enter) => "enter",
            Key::Named(NamedKey::Escape) => "escape",
            Key::Named(NamedKey::Tab) => "tab",
            Key::Named(NamedKey::Backspace) => "backspace",
            Key::Named(NamedKey::Delete) => "delete",
            Key::Named(NamedKey::ArrowUp) => "up",
            Key::Named(NamedKey::ArrowDown) => "down",
            Key::Named(NamedKey::ArrowLeft) => "left",
            Key::Named(NamedKey::ArrowRight) => "right",
            Key::Named(NamedKey::Home) => "home",
            Key::Named(NamedKey::End) => "end",
            Key::Character(c) => c,
        };
        parts.push(key_str);

        write!(f, "{}", parts.join("-"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let ks = Keystroke::parse("a").unwrap();
        assert_eq!(ks.key, Key::Character("a".to_string()));
        assert!(!ks.modifiers.meta);
        assert!(!ks.modifiers.ctrl);
        assert!(!ks.modifiers.alt);
        assert!(!ks.modifiers.shift);
    }

    #[test]
    fn test_parse_with_modifiers() {
        let ks = Keystroke::parse("cmd-shift-s").unwrap();
        assert_eq!(ks.key, Key::Character("s".to_string()));
        assert!(ks.modifiers.meta);
        assert!(ks.modifiers.shift);
        assert!(!ks.modifiers.ctrl);
        assert!(!ks.modifiers.alt);
    }

    #[test]
    fn test_parse_named_key() {
        let ks = Keystroke::parse("escape").unwrap();
        assert_eq!(ks.key, Key::Named(NamedKey::Escape));

        let ks = Keystroke::parse("ctrl-enter").unwrap();
        assert_eq!(ks.key, Key::Named(NamedKey::Enter));
        assert!(ks.modifiers.ctrl);
    }

    #[test]
    fn test_parse_arrow_keys() {
        assert_eq!(
            Keystroke::parse("up").unwrap().key,
            Key::Named(NamedKey::ArrowUp)
        );
        assert_eq!(
            Keystroke::parse("down").unwrap().key,
            Key::Named(NamedKey::ArrowDown)
        );
        assert_eq!(
            Keystroke::parse("left").unwrap().key,
            Key::Named(NamedKey::ArrowLeft)
        );
        assert_eq!(
            Keystroke::parse("right").unwrap().key,
            Key::Named(NamedKey::ArrowRight)
        );
    }

    #[test]
    fn test_parse_no_key_error() {
        let result = Keystroke::parse("cmd-shift");
        assert!(matches!(result, Err(KeystrokeParseError::NoKey)));
    }

    #[test]
    fn test_matches() {
        let ks = Keystroke::parse("cmd-s").unwrap();

        // Should match
        let modifiers = Modifiers {
            meta: true,
            ..Default::default()
        };
        assert_eq!(
            ks.matches(&Key::Character("s".to_string()), &modifiers),
            KeystrokeMatch::Matched
        );

        // Should not match - wrong modifiers
        let modifiers = Modifiers::default();
        assert_eq!(
            ks.matches(&Key::Character("s".to_string()), &modifiers),
            KeystrokeMatch::None
        );

        // Should not match - wrong key
        let modifiers = Modifiers {
            meta: true,
            ..Default::default()
        };
        assert_eq!(
            ks.matches(&Key::Character("a".to_string()), &modifiers),
            KeystrokeMatch::None
        );
    }

    #[test]
    fn test_display() {
        let ks = Keystroke::parse("cmd-shift-s").unwrap();
        assert_eq!(ks.to_string(), "cmd-shift-s");

        let ks = Keystroke::parse("escape").unwrap();
        assert_eq!(ks.to_string(), "escape");

        let ks = Keystroke::parse("ctrl-alt-delete").unwrap();
        assert_eq!(ks.to_string(), "ctrl-alt-delete");
    }
}
