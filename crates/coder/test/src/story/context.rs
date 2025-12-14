//! TestContext - provides access to test resources.

use crate::actions::UserActions;
use crate::fixtures::{Fixture, FixtureRegistry};
use crate::harness::TestHarness;
use std::collections::HashMap;
use std::time::Duration;

/// Context provided to story steps.
///
/// TestContext provides access to:
/// - Fixtures for domain/UI state
/// - TestHarness for headless widget testing
/// - UserActions for simulating input
/// - Assertions and utilities
pub struct TestContext {
    /// Test harness for headless widget testing.
    harness: TestHarness,
    /// Fixture registry.
    fixtures: FixtureRegistry,
    /// Arbitrary data storage for sharing between steps.
    data: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl TestContext {
    /// Create a new test context.
    pub fn new() -> Self {
        Self {
            harness: TestHarness::new(),
            fixtures: FixtureRegistry::new(),
            data: HashMap::new(),
        }
    }

    /// Get a reference to the test harness.
    pub fn harness(&self) -> &TestHarness {
        &self.harness
    }

    /// Get a mutable reference to the test harness.
    pub fn harness_mut(&mut self) -> &mut TestHarness {
        &mut self.harness
    }

    /// Get or create a fixture of the given type.
    ///
    /// Fixtures are lazily initialized and cached for the duration of the test.
    pub fn fixture<F: Fixture + Default + 'static>(&mut self) -> &mut F {
        self.fixtures.get_or_create::<F>()
    }

    /// Create a UserActions builder for simulating input.
    pub fn actions(&mut self) -> UserActions<'_> {
        UserActions::new(&mut self.harness)
    }

    /// Store arbitrary data that can be retrieved in later steps.
    pub fn store<T: Send + Sync + 'static>(&mut self, key: impl Into<String>, value: T) {
        self.data.insert(key.into(), Box::new(value));
    }

    /// Retrieve stored data by key.
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.data.get(key).and_then(|v| v.downcast_ref())
    }

    /// Retrieve stored data mutably.
    pub fn get_mut<T: 'static>(&mut self, key: &str) -> Option<&mut T> {
        self.data.get_mut(key).and_then(|v| v.downcast_mut())
    }

    /// Wait for a duration (useful for async operations to settle).
    pub fn wait_for(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    /// Wait for a condition to become true with timeout.
    pub fn wait_until<F>(&self, timeout: Duration, condition: F) -> bool
    where
        F: Fn() -> bool,
    {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(10);

        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            std::thread::sleep(poll_interval);
        }

        false
    }

    /// Assert that the scene contains the specified text.
    pub fn assert_scene_contains_text(&self, text: &str) {
        use crate::assertions::SceneAssertions;
        let scene = self.harness.scene();
        assert!(
            scene.contains_text(text),
            "Expected scene to contain text '{}', but it didn't.\nScene has {} text runs.",
            text,
            scene.text_run_count()
        );
    }

    /// Assert that the scene does NOT contain the specified text.
    pub fn assert_scene_not_contains_text(&self, text: &str) {
        use crate::assertions::SceneAssertions;
        let scene = self.harness.scene();
        assert!(
            !scene.contains_text(text),
            "Expected scene NOT to contain text '{}', but it did.",
            text
        );
    }

    /// Get the captured scene for custom assertions.
    pub fn scene(&self) -> &wgpui::Scene {
        self.harness.scene()
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_store_retrieve() {
        let mut cx = TestContext::new();

        cx.store("count", 42i32);
        cx.store("name", "test".to_string());

        assert_eq!(cx.get::<i32>("count"), Some(&42));
        assert_eq!(cx.get::<String>("name"), Some(&"test".to_string()));
        assert_eq!(cx.get::<i32>("missing"), None);
    }

    #[test]
    fn test_context_wait_until() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let cx = TestContext::new();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        // Spawn a thread that will set the flag after a short delay
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            flag_clone.store(true, Ordering::SeqCst);
        });

        let result = cx.wait_until(Duration::from_millis(200), || {
            flag.load(Ordering::SeqCst)
        });

        assert!(result, "Expected condition to become true");
    }

    #[test]
    fn test_context_wait_until_timeout() {
        let cx = TestContext::new();

        let result = cx.wait_until(Duration::from_millis(50), || false);

        assert!(!result, "Expected timeout");
    }
}
