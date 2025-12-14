//! Fixture registry for managing test fixtures.

use super::Fixture;
use std::any::TypeId;
use std::collections::HashMap;

/// Registry for managing test fixtures.
///
/// Fixtures are lazily created on first access and cached
/// for the duration of the test.
pub struct FixtureRegistry {
    /// Stored fixtures by type ID.
    fixtures: HashMap<TypeId, Box<dyn Fixture>>,
}

impl FixtureRegistry {
    /// Create a new fixture registry.
    pub fn new() -> Self {
        Self {
            fixtures: HashMap::new(),
        }
    }

    /// Get or create a fixture of the given type.
    ///
    /// If the fixture doesn't exist, it will be created using Default
    /// and its setup() method will be called.
    pub fn get_or_create<F: Fixture + Default + 'static>(&mut self) -> &mut F {
        let type_id = TypeId::of::<F>();

        if !self.fixtures.contains_key(&type_id) {
            let mut fixture = F::default();
            fixture.setup();
            self.fixtures.insert(type_id, Box::new(fixture));
        }

        self.fixtures
            .get_mut(&type_id)
            .and_then(|f| f.as_any_mut().downcast_mut::<F>())
            .expect("Fixture type mismatch")
    }

    /// Check if a fixture exists.
    pub fn has<F: 'static>(&self) -> bool {
        self.fixtures.contains_key(&TypeId::of::<F>())
    }

    /// Get a fixture if it exists.
    pub fn get<F: 'static>(&self) -> Option<&F> {
        self.fixtures
            .get(&TypeId::of::<F>())
            .and_then(|f| f.as_any().downcast_ref::<F>())
    }

    /// Get a mutable fixture if it exists.
    pub fn get_mut<F: 'static>(&mut self) -> Option<&mut F> {
        self.fixtures
            .get_mut(&TypeId::of::<F>())
            .and_then(|f| f.as_any_mut().downcast_mut::<F>())
    }

    /// Remove a fixture.
    pub fn remove<F: 'static>(&mut self) -> Option<Box<dyn Fixture>> {
        self.fixtures.remove(&TypeId::of::<F>())
    }

    /// Clear all fixtures, calling teardown on each.
    pub fn teardown_all(&mut self) {
        for fixture in self.fixtures.values_mut() {
            fixture.teardown();
        }
        self.fixtures.clear();
    }

    /// Get the number of registered fixtures.
    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}

impl Default for FixtureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FixtureRegistry {
    fn drop(&mut self) {
        self.teardown_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestFixture {
        value: i32,
    }

    #[test]
    fn test_fixture_registry_get_or_create() {
        let mut registry = FixtureRegistry::new();

        // First access creates the fixture
        let fixture = registry.get_or_create::<TestFixture>();
        fixture.value = 42;

        // Second access returns the same fixture
        let fixture = registry.get_or_create::<TestFixture>();
        assert_eq!(fixture.value, 42);
    }

    #[test]
    fn test_fixture_registry_has() {
        let mut registry = FixtureRegistry::new();

        assert!(!registry.has::<TestFixture>());

        registry.get_or_create::<TestFixture>();

        assert!(registry.has::<TestFixture>());
    }

    #[test]
    fn test_fixture_registry_get() {
        let mut registry = FixtureRegistry::new();

        assert!(registry.get::<TestFixture>().is_none());

        registry.get_or_create::<TestFixture>().value = 100;

        let fixture = registry.get::<TestFixture>();
        assert!(fixture.is_some());
        assert_eq!(fixture.unwrap().value, 100);
    }

    #[test]
    fn test_fixture_registry_remove() {
        let mut registry = FixtureRegistry::new();
        registry.get_or_create::<TestFixture>();

        assert!(registry.has::<TestFixture>());

        registry.remove::<TestFixture>();

        assert!(!registry.has::<TestFixture>());
    }

    #[test]
    fn test_fixture_registry_len() {
        let mut registry = FixtureRegistry::new();

        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        registry.get_or_create::<TestFixture>();

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }
}
