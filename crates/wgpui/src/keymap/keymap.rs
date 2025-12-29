//! Keymap with precedence-based binding resolution.

use super::KeyContext;
use crate::action::{AnyAction, KeyBinding};
use crate::{Key, Modifiers};

/// Manages keybindings and resolves them to actions.
///
/// The keymap stores bindings and resolves keystrokes to actions based on
/// the current context. Precedence rules:
///
/// 1. **Context depth** - More specific (deeper) context matches win
/// 2. **Binding order** - Later bindings override earlier ones
///
/// # Example
///
/// ```ignore
/// let mut keymap = Keymap::new();
///
/// // Global escape binding
/// keymap.add(KeyBinding::new("escape", Cancel).unwrap());
///
/// // Modal-specific escape binding (takes precedence when in Modal context)
/// keymap.add(KeyBinding::new("escape", CloseModal).unwrap().with_context("Modal"));
/// ```
pub struct Keymap {
    /// All bindings, ordered by precedence (later = higher priority).
    bindings: Vec<KeyBinding>,
}

impl Keymap {
    /// Create a new empty keymap.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Add a keybinding.
    pub fn add(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Add multiple bindings (e.g., from defaults or user config).
    pub fn add_bindings(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        self.bindings.extend(bindings);
    }

    /// Remove all bindings.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Get the number of bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Check if the keymap is empty.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Find the best matching action for a keystroke in the given context.
    ///
    /// Returns the action from the binding with the highest precedence:
    /// 1. Most specific context match (deeper context depth)
    /// 2. Later binding in list (user overrides defaults)
    ///
    /// Returns `None` if no binding matches.
    pub fn match_keystroke(
        &self,
        key: &Key,
        modifiers: &Modifiers,
        context: &KeyContext,
    ) -> Option<Box<dyn AnyAction>> {
        let mut best_match: Option<(usize, usize, &KeyBinding)> = None;

        for (index, binding) in self.bindings.iter().enumerate() {
            // Check if keystroke matches
            if !binding.matches(key, modifiers) {
                continue;
            }

            // Check context match
            if !context.matches_binding_context(binding.context.as_deref()) {
                continue;
            }

            // Calculate context depth for this binding
            let depth = binding
                .context
                .as_ref()
                .and_then(|ctx| context.depth_of(ctx))
                .unwrap_or(0);

            // Keep the best match:
            // - Prefer higher depth (more specific context)
            // - If same depth, prefer higher index (later binding)
            match &best_match {
                None => best_match = Some((depth, index, binding)),
                Some((best_depth, best_index, _)) => {
                    if depth > *best_depth || (depth == *best_depth && index > *best_index) {
                        best_match = Some((depth, index, binding));
                    }
                }
            }
        }

        best_match.map(|(_, _, binding)| binding.action.boxed_clone())
    }

    /// Get all bindings (for display in settings or command palette).
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Find the keybinding string for an action by name (for UI display).
    ///
    /// Returns the first matching binding's keystroke string.
    pub fn binding_for_action(&self, action_name: &str) -> Option<String> {
        self.bindings
            .iter()
            .find(|b| b.action.name() == action_name)
            .map(|b| b.keystroke_string())
    }

    /// Find all keybindings for an action by name.
    pub fn bindings_for_action(&self, action_name: &str) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|b| b.action.name() == action_name)
            .collect()
    }

    /// Remove bindings for a specific action.
    pub fn remove_action_bindings(&mut self, action_name: &str) {
        self.bindings.retain(|b| b.action.name() != action_name);
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, NoAction};
    use crate::NamedKey;

    #[derive(Debug, Clone, Default)]
    struct ActionA;
    impl Action for ActionA {
        fn name() -> &'static str {
            "ActionA"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct ActionB;
    impl Action for ActionB {
        fn name() -> &'static str {
            "ActionB"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct ActionC;
    impl Action for ActionC {
        fn name() -> &'static str {
            "ActionC"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    fn escape_key() -> (Key, Modifiers) {
        (Key::Named(NamedKey::Escape), Modifiers::default())
    }

    #[test]
    fn test_new_keymap() {
        let keymap = Keymap::new();
        assert!(keymap.is_empty());
        assert_eq!(keymap.len(), 0);
    }

    #[test]
    fn test_add_binding() {
        let mut keymap = Keymap::new();
        keymap.add(KeyBinding::new("escape", NoAction).unwrap());
        assert_eq!(keymap.len(), 1);
    }

    #[test]
    fn test_simple_match() {
        let mut keymap = Keymap::new();
        keymap.add(KeyBinding::new("escape", ActionA).unwrap());

        let context = KeyContext::new();
        let (key, mods) = escape_key();

        let action = keymap.match_keystroke(&key, &mods, &context);
        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "ActionA");
    }

    #[test]
    fn test_no_match() {
        let mut keymap = Keymap::new();
        keymap.add(KeyBinding::new("enter", ActionA).unwrap());

        let context = KeyContext::new();
        let (key, mods) = escape_key();

        let action = keymap.match_keystroke(&key, &mods, &context);
        assert!(action.is_none());
    }

    #[test]
    fn test_context_match() {
        let mut keymap = Keymap::new();
        keymap.add(
            KeyBinding::new("escape", ActionA)
                .unwrap()
                .with_context("Modal"),
        );

        // Without context - no match
        let context = KeyContext::new();
        let (key, mods) = escape_key();
        assert!(keymap.match_keystroke(&key, &mods, &context).is_none());

        // With context - match
        let mut context = KeyContext::new();
        context.push("Modal");
        let action = keymap.match_keystroke(&key, &mods, &context);
        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "ActionA");
    }

    #[test]
    fn test_context_precedence() {
        let mut keymap = Keymap::new();

        // Global binding
        keymap.add(KeyBinding::new("escape", ActionA).unwrap());

        // Editor-specific binding
        keymap.add(
            KeyBinding::new("escape", ActionB)
                .unwrap()
                .with_context("Editor"),
        );

        // TextInput-specific binding
        keymap.add(
            KeyBinding::new("escape", ActionC)
                .unwrap()
                .with_context("TextInput"),
        );

        let (key, mods) = escape_key();

        // Global context - ActionA wins (no context requirement)
        let context = KeyContext::new();
        let action = keymap.match_keystroke(&key, &mods, &context);
        assert_eq!(action.unwrap().name(), "ActionA");

        // Editor context - ActionB wins (more specific)
        let mut context = KeyContext::new();
        context.push("Editor");
        let action = keymap.match_keystroke(&key, &mods, &context);
        assert_eq!(action.unwrap().name(), "ActionB");

        // Editor > TextInput context - ActionC wins (deepest)
        let mut context = KeyContext::new();
        context.push("Editor");
        context.push("TextInput");
        let action = keymap.match_keystroke(&key, &mods, &context);
        assert_eq!(action.unwrap().name(), "ActionC");
    }

    #[test]
    fn test_later_binding_wins() {
        let mut keymap = Keymap::new();

        // Earlier binding
        keymap.add(KeyBinding::new("escape", ActionA).unwrap());

        // Later binding (same key, same context) - should win
        keymap.add(KeyBinding::new("escape", ActionB).unwrap());

        let context = KeyContext::new();
        let (key, mods) = escape_key();

        let action = keymap.match_keystroke(&key, &mods, &context);
        assert_eq!(action.unwrap().name(), "ActionB");
    }

    #[test]
    fn test_binding_for_action() {
        let mut keymap = Keymap::new();
        keymap.add(KeyBinding::new("cmd-s", ActionA).unwrap());
        keymap.add(KeyBinding::new("escape", ActionB).unwrap());

        assert_eq!(keymap.binding_for_action("ActionA"), Some("cmd-s".to_string()));
        assert_eq!(
            keymap.binding_for_action("ActionB"),
            Some("escape".to_string())
        );
        assert_eq!(keymap.binding_for_action("ActionC"), None);
    }

    #[test]
    fn test_remove_action_bindings() {
        let mut keymap = Keymap::new();
        keymap.add(KeyBinding::new("escape", ActionA).unwrap());
        keymap.add(KeyBinding::new("cmd-s", ActionA).unwrap());
        keymap.add(KeyBinding::new("enter", ActionB).unwrap());

        assert_eq!(keymap.len(), 3);

        keymap.remove_action_bindings("ActionA");
        assert_eq!(keymap.len(), 1);
        assert_eq!(keymap.bindings()[0].action.name(), "ActionB");
    }
}
