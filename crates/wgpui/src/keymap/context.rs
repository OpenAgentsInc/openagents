//! Key context for scoped keybindings.

use smallvec::SmallVec;

/// Context stack for matching keybindings.
///
/// Contexts form a stack representing the current UI state hierarchy.
/// For example: `["Window", "Editor", "TextInput"]` indicates a text input
/// inside an editor inside the main window.
///
/// When matching keybindings, more specific contexts (deeper in the stack)
/// take precedence over less specific ones.
///
/// # Example
///
/// ```ignore
/// let mut context = KeyContext::new();
/// context.push("Window");
/// context.push("Editor");
/// context.push("TextInput");
///
/// // A binding with context "TextInput" wins over one with "Editor"
/// // A binding with no context matches anywhere
/// ```
#[derive(Debug, Clone, Default)]
pub struct KeyContext {
    /// Stack of context identifiers, from root to leaf.
    identifiers: SmallVec<[String; 4]>,
}

impl KeyContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a context identifier onto the stack.
    ///
    /// This should be called when entering a component that defines
    /// a keybinding context (e.g., modal, editor, text input).
    pub fn push(&mut self, identifier: impl Into<String>) {
        self.identifiers.push(identifier.into());
    }

    /// Pop the most recent context identifier.
    ///
    /// This should be called when leaving a component that defines
    /// a keybinding context.
    pub fn pop(&mut self) -> Option<String> {
        self.identifiers.pop()
    }

    /// Clear all context identifiers.
    pub fn clear(&mut self) {
        self.identifiers.clear();
    }

    /// Check if this context contains the given identifier.
    pub fn contains(&self, identifier: &str) -> bool {
        self.identifiers.iter().any(|id| id == identifier)
    }

    /// Get the depth (number of contexts pushed).
    pub fn depth(&self) -> usize {
        self.identifiers.len()
    }

    /// Check if the context is empty.
    pub fn is_empty(&self) -> bool {
        self.identifiers.is_empty()
    }

    /// Get the most recent (deepest) context identifier.
    pub fn current(&self) -> Option<&str> {
        self.identifiers.last().map(|s| s.as_str())
    }

    /// Check if a binding's context requirement matches this context.
    ///
    /// Returns `true` if:
    /// - The binding has no context requirement (matches anywhere)
    /// - The binding's context is present in this context stack
    pub fn matches_binding_context(&self, binding_context: Option<&str>) -> bool {
        match binding_context {
            None => true, // No requirement, always matches
            Some(required) => self.contains(required),
        }
    }

    /// Get the depth at which a context identifier appears.
    ///
    /// Returns `None` if the identifier is not in the context.
    /// Higher values indicate more specific (deeper) contexts.
    pub fn depth_of(&self, identifier: &str) -> Option<usize> {
        self.identifiers
            .iter()
            .position(|id| id == identifier)
            .map(|pos| pos + 1) // 1-indexed depth
    }

    /// Iterate over identifiers from root to leaf.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.identifiers.iter().map(|s| s.as_str())
    }

    /// Iterate over identifiers from leaf to root.
    pub fn iter_rev(&self) -> impl Iterator<Item = &str> {
        self.identifiers.iter().rev().map(|s| s.as_str())
    }
}

impl From<&str> for KeyContext {
    fn from(identifier: &str) -> Self {
        let mut ctx = Self::new();
        ctx.push(identifier);
        ctx
    }
}

impl From<String> for KeyContext {
    fn from(identifier: String) -> Self {
        let mut ctx = Self::new();
        ctx.push(identifier);
        ctx
    }
}

impl std::fmt::Display for KeyContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.identifiers.join(" > "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let ctx = KeyContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn test_push_pop() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");

        assert_eq!(ctx.depth(), 2);
        assert_eq!(ctx.current(), Some("Editor"));

        let popped = ctx.pop();
        assert_eq!(popped, Some("Editor".to_string()));
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current(), Some("Window"));
    }

    #[test]
    fn test_contains() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");
        ctx.push("TextInput");

        assert!(ctx.contains("Window"));
        assert!(ctx.contains("Editor"));
        assert!(ctx.contains("TextInput"));
        assert!(!ctx.contains("Modal"));
    }

    #[test]
    fn test_matches_binding_context() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");

        // No requirement matches anything
        assert!(ctx.matches_binding_context(None));

        // Present context matches
        assert!(ctx.matches_binding_context(Some("Editor")));
        assert!(ctx.matches_binding_context(Some("Window")));

        // Missing context doesn't match
        assert!(!ctx.matches_binding_context(Some("Modal")));
    }

    #[test]
    fn test_depth_of() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");
        ctx.push("TextInput");

        assert_eq!(ctx.depth_of("Window"), Some(1));
        assert_eq!(ctx.depth_of("Editor"), Some(2));
        assert_eq!(ctx.depth_of("TextInput"), Some(3));
        assert_eq!(ctx.depth_of("Modal"), None);
    }

    #[test]
    fn test_from_str() {
        let ctx = KeyContext::from("Editor");
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current(), Some("Editor"));
    }

    #[test]
    fn test_clear() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");
        ctx.clear();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_display() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");
        assert_eq!(ctx.to_string(), "[Window > Editor]");
    }

    #[test]
    fn test_iter() {
        let mut ctx = KeyContext::new();
        ctx.push("Window");
        ctx.push("Editor");
        ctx.push("TextInput");

        let forward: Vec<_> = ctx.iter().collect();
        assert_eq!(forward, vec!["Window", "Editor", "TextInput"]);

        let reverse: Vec<_> = ctx.iter_rev().collect();
        assert_eq!(reverse, vec!["TextInput", "Editor", "Window"]);
    }
}
