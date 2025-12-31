//! Key binding connecting keystrokes to actions.

use super::{AnyAction, Keystroke, KeystrokeMatch, KeystrokeParseError};
use crate::{Key, Modifiers};

/// A binding from a keystroke sequence to an action.
pub struct KeyBinding {
    /// Keystrokes that trigger this binding (usually 1, can be sequence).
    pub keystrokes: Vec<Keystroke>,
    /// Action to dispatch when triggered.
    pub action: Box<dyn AnyAction>,
    /// Context predicate (e.g., "Editor", "Modal", "TextInput").
    /// If None, the binding matches in any context.
    pub context: Option<String>,
}

impl Clone for KeyBinding {
    fn clone(&self) -> Self {
        Self {
            keystrokes: self.keystrokes.clone(),
            action: self.action.boxed_clone(),
            context: self.context.clone(),
        }
    }
}

impl KeyBinding {
    /// Create a new keybinding from a keystroke string and action.
    ///
    /// # Example
    /// ```ignore
    /// let binding = KeyBinding::new("cmd-s", Save).unwrap();
    /// ```
    pub fn new<A: super::Action>(keystroke: &str, action: A) -> Result<Self, KeystrokeParseError> {
        Ok(Self {
            keystrokes: vec![Keystroke::parse(keystroke)?],
            action: Box::new(action),
            context: None,
        })
    }

    /// Create a keybinding with a pre-parsed keystroke.
    pub fn with_keystroke<A: super::Action>(keystroke: Keystroke, action: A) -> Self {
        Self {
            keystrokes: vec![keystroke],
            action: Box::new(action),
            context: None,
        }
    }

    /// Create a keybinding with a keystroke sequence (for chords like "ctrl-k ctrl-c").
    pub fn with_sequence<A: super::Action>(keystrokes: Vec<Keystroke>, action: A) -> Self {
        Self {
            keystrokes,
            action: Box::new(action),
            context: None,
        }
    }

    /// Set a context requirement for this binding.
    ///
    /// The binding will only match when the context stack contains this identifier.
    ///
    /// # Example
    /// ```ignore
    /// let binding = KeyBinding::new("escape", Cancel)
    ///     .unwrap()
    ///     .with_context("Modal");
    /// ```
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Check if this binding's keystroke(s) match the input.
    ///
    /// Currently only supports single keystrokes. Multi-key sequences
    /// will return false and are reserved for future implementation.
    pub fn matches(&self, key: &Key, modifiers: &Modifiers) -> bool {
        // For now, only support single keystrokes
        if self.keystrokes.len() != 1 {
            return false;
        }

        matches!(
            self.keystrokes[0].matches(key, modifiers),
            KeystrokeMatch::Matched
        )
    }

    /// Get the display string for this binding's keystrokes.
    pub fn keystroke_string(&self) -> String {
        self.keystrokes
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl std::fmt::Debug for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyBinding")
            .field("keystrokes", &self.keystroke_string())
            .field("action", &self.action.name())
            .field("context", &self.context)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, NoAction};

    #[derive(Debug, Clone, Default)]
    struct TestAction;

    impl Action for TestAction {
        fn name() -> &'static str {
            "test::TestAction"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_new_binding() {
        let binding = KeyBinding::new("cmd-s", TestAction).unwrap();
        assert_eq!(binding.action.name(), "test::TestAction");
        assert!(binding.context.is_none());
    }

    #[test]
    fn test_with_context() {
        let binding = KeyBinding::new("escape", TestAction)
            .unwrap()
            .with_context("Modal");
        assert_eq!(binding.context, Some("Modal".to_string()));
    }

    #[test]
    fn test_matches() {
        let binding = KeyBinding::new("cmd-s", TestAction).unwrap();

        let modifiers = Modifiers {
            meta: true,
            ..Default::default()
        };
        assert!(binding.matches(&Key::Character("s".to_string()), &modifiers));

        let modifiers = Modifiers::default();
        assert!(!binding.matches(&Key::Character("s".to_string()), &modifiers));
    }

    #[test]
    fn test_keystroke_string() {
        let binding = KeyBinding::new("cmd-shift-s", TestAction).unwrap();
        assert_eq!(binding.keystroke_string(), "cmd-shift-s");
    }

    #[test]
    fn test_debug_format() {
        let binding = KeyBinding::new("cmd-s", NoAction)
            .unwrap()
            .with_context("Editor");
        let debug = format!("{:?}", binding);
        assert!(debug.contains("cmd-s"));
        assert!(debug.contains("NoAction"));
        assert!(debug.contains("Editor"));
    }
}
