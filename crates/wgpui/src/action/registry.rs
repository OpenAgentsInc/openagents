//! Action registry for name lookups and binding storage.

use super::{Action, ActionId, AnyAction, KeyBinding};
use std::collections::HashMap;

/// Builds actions from their registered name.
type ActionBuilder = Box<dyn Fn() -> Box<dyn AnyAction> + Send + Sync>;

/// Registry of known actions and their default bindings.
///
/// The registry serves two purposes:
/// 1. Map action names to builders for deserializing keymaps
/// 2. Store default keybindings that can be loaded into a keymap
///
/// # Example
/// ```ignore
/// let mut registry = ActionRegistry::new();
/// registry.register::<Save>();
/// registry.add_binding(KeyBinding::new("cmd-s", Save).unwrap());
/// ```
pub struct ActionRegistry {
    /// Map from action name to builder.
    builders: HashMap<&'static str, ActionBuilder>,
    /// Map from ActionId to name.
    names: HashMap<ActionId, &'static str>,
    /// Default keybindings.
    default_bindings: Vec<KeyBinding>,
}

impl ActionRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
            names: HashMap::new(),
            default_bindings: Vec::new(),
        }
    }

    /// Register an action type with the registry.
    ///
    /// The action must implement `Default` to be buildable from its name.
    pub fn register<A: Action + Default>(&mut self) {
        let name = A::name();
        let id = std::any::TypeId::of::<A>();

        self.builders
            .insert(name, Box::new(|| Box::new(A::default())));
        self.names.insert(id, name);
    }

    /// Register an action with a custom builder function.
    pub fn register_with_builder<A: Action>(
        &mut self,
        builder: impl Fn() -> A + Send + Sync + 'static,
    ) {
        let name = A::name();
        let id = std::any::TypeId::of::<A>();

        self.builders
            .insert(name, Box::new(move || Box::new(builder())));
        self.names.insert(id, name);
    }

    /// Add a default keybinding.
    pub fn add_binding(&mut self, binding: KeyBinding) {
        self.default_bindings.push(binding);
    }

    /// Add multiple default keybindings.
    pub fn add_bindings(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        self.default_bindings.extend(bindings);
    }

    /// Get action name from its type ID.
    pub fn name(&self, id: ActionId) -> Option<&'static str> {
        self.names.get(&id).copied()
    }

    /// Build an action from its name.
    pub fn build(&self, name: &str) -> Option<Box<dyn AnyAction>> {
        self.builders.get(name).map(|b| b())
    }

    /// Check if an action is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.builders.contains_key(name)
    }

    /// Get all registered action names.
    pub fn action_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.names.values().copied()
    }

    /// Get all default bindings.
    pub fn default_bindings(&self) -> &[KeyBinding] {
        &self.default_bindings
    }

    /// Take ownership of all default bindings.
    pub fn take_default_bindings(&mut self) -> Vec<KeyBinding> {
        std::mem::take(&mut self.default_bindings)
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[derive(Debug, Clone)]
    struct ActionWithData {
        value: i32,
    }

    impl Action for ActionWithData {
        fn name() -> &'static str {
            "test::ActionWithData"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_register_and_build() {
        let mut registry = ActionRegistry::new();
        registry.register::<TestAction>();

        assert!(registry.is_registered("test::TestAction"));
        let action = registry.build("test::TestAction");
        assert!(action.is_some());
        assert_eq!(action.unwrap().name(), "test::TestAction");
    }

    #[test]
    fn test_register_with_builder() {
        let mut registry = ActionRegistry::new();
        registry.register_with_builder(|| ActionWithData { value: 42 });

        let action = registry.build("test::ActionWithData").unwrap();
        let typed = action.as_any().downcast_ref::<ActionWithData>().unwrap();
        assert_eq!(typed.value, 42);
    }

    #[test]
    fn test_name_lookup() {
        let mut registry = ActionRegistry::new();
        registry.register::<TestAction>();

        let id = std::any::TypeId::of::<TestAction>();
        assert_eq!(registry.name(id), Some("test::TestAction"));
    }

    #[test]
    fn test_default_bindings() {
        let mut registry = ActionRegistry::new();
        registry.add_binding(KeyBinding::new("cmd-s", TestAction).unwrap());
        registry.add_binding(KeyBinding::new("cmd-z", TestAction).unwrap());

        assert_eq!(registry.default_bindings().len(), 2);

        let bindings = registry.take_default_bindings();
        assert_eq!(bindings.len(), 2);
        assert!(registry.default_bindings().is_empty());
    }

    #[test]
    fn test_action_names() {
        let mut registry = ActionRegistry::new();
        registry.register::<TestAction>();

        let names: Vec<_> = registry.action_names().collect();
        assert!(names.contains(&"test::TestAction"));
    }
}
