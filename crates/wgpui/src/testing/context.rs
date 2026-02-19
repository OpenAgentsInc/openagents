//! Test context and component registry.
//!
//! Provides infrastructure for tracking components during testing.

use crate::{Bounds, ComponentId};
use std::collections::HashMap;

/// Registry that tracks component locations for testing.
///
/// During painting, components register their bounds so tests can
/// find elements by ID or text for interaction.
#[derive(Default, Debug)]
pub struct ComponentRegistry {
    /// Map from ComponentId to bounds.
    id_bounds: HashMap<ComponentId, Bounds>,
    /// Map from visible text to bounds.
    text_bounds: HashMap<String, Vec<Bounds>>,
}

impl ComponentRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all registered components (call at start of each paint).
    pub fn clear(&mut self) {
        self.id_bounds.clear();
        self.text_bounds.clear();
    }

    /// Register a component by its ID and bounds.
    pub fn register_id(&mut self, id: ComponentId, bounds: Bounds) {
        self.id_bounds.insert(id, bounds);
    }

    /// Register text content at specific bounds.
    pub fn register_text(&mut self, text: &str, bounds: Bounds) {
        self.text_bounds
            .entry(text.to_string())
            .or_default()
            .push(bounds);
    }

    /// Look up bounds by ComponentId.
    pub fn find_by_id(&self, id: ComponentId) -> Option<Bounds> {
        self.id_bounds.get(&id).copied()
    }

    /// Look up bounds by text content. Returns first match.
    pub fn find_by_text(&self, text: &str) -> Option<Bounds> {
        self.text_bounds.get(text).and_then(|v| v.first()).copied()
    }

    /// Look up all bounds matching a text query.
    pub fn find_all_by_text(&self, text: &str) -> Vec<Bounds> {
        self.text_bounds.get(text).cloned().unwrap_or_default()
    }

    /// Find the center point of an element by ID.
    pub fn center_of_id(&self, id: ComponentId) -> Option<crate::Point> {
        self.find_by_id(id).map(|b| b.center())
    }

    /// Find the center point of an element by text.
    pub fn center_of_text(&self, text: &str) -> Option<crate::Point> {
        self.find_by_text(text).map(|b| b.center())
    }

    /// Get the number of registered components.
    pub fn component_count(&self) -> usize {
        self.id_bounds.len()
    }

    /// Get the number of registered text entries.
    pub fn text_count(&self) -> usize {
        self.text_bounds.len()
    }
}

/// Test context passed during test execution.
///
/// Provides access to the component registry and test state.
#[derive(Default)]
pub struct TestContext {
    /// Registry of component locations.
    pub registry: ComponentRegistry,
    /// Current test name.
    pub test_name: String,
    /// Current step index (0-based).
    pub current_step: usize,
    /// Total number of steps.
    pub total_steps: usize,
    /// Whether the test is currently paused.
    pub paused: bool,
}

impl TestContext {
    /// Create a new test context with a name.
    pub fn new(test_name: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            ..Default::default()
        }
    }

    /// Get the human-readable progress string.
    pub fn progress_string(&self) -> String {
        format!(
            "Step {}/{} - {}",
            self.current_step + 1,
            self.total_steps,
            self.test_name
        )
    }

    /// Update progress.
    pub fn set_progress(&mut self, current: usize, total: usize) {
        self.current_step = current;
        self.total_steps = total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bounds() -> Bounds {
        Bounds::new(10.0, 20.0, 100.0, 50.0)
    }

    #[test]
    fn test_registry_register_and_find_id() {
        let mut registry = ComponentRegistry::new();
        let bounds = test_bounds();

        registry.register_id(42, bounds);

        assert_eq!(registry.find_by_id(42), Some(bounds));
        assert_eq!(registry.find_by_id(999), None);
    }

    #[test]
    fn test_registry_register_and_find_text() {
        let mut registry = ComponentRegistry::new();
        let bounds = test_bounds();

        registry.register_text("Hello World", bounds);

        assert_eq!(registry.find_by_text("Hello World"), Some(bounds));
        assert_eq!(registry.find_by_text("Not Found"), None);
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = ComponentRegistry::new();
        registry.register_id(1, test_bounds());
        registry.register_text("Hello", test_bounds());

        assert_eq!(registry.component_count(), 1);
        assert_eq!(registry.text_count(), 1);

        registry.clear();

        assert_eq!(registry.component_count(), 0);
        assert_eq!(registry.text_count(), 0);
    }

    #[test]
    fn test_registry_center_of_id() {
        let mut registry = ComponentRegistry::new();
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

        registry.register_id(1, bounds);

        let center = registry.center_of_id(1).unwrap();
        assert_eq!(center.x, 50.0);
        assert_eq!(center.y, 50.0);
    }

    #[test]
    fn test_context_progress() {
        let mut ctx = TestContext::new("Login Test");
        ctx.set_progress(2, 5);

        assert_eq!(ctx.progress_string(), "Step 3/5 - Login Test");
    }
}
