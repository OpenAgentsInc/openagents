//! Action dispatch utilities.
//!
//! This module provides helpers for dispatching actions through the UI tree.

use super::{ActionId, AnyAction};
use std::any::Any;
use std::collections::HashMap;

/// Phase of action dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DispatchPhase {
    /// Bubble phase: from focused element up to ancestors.
    #[default]
    Bubble,
    /// Capture phase: from root down to focused element (future use).
    Capture,
}

/// Type alias for action handler closures.
pub type ActionHandler = Box<dyn FnMut(&dyn Any) -> bool + 'static>;

/// Stores action listeners for a component.
#[derive(Default)]
pub struct ActionListeners {
    /// Map from ActionId to handler.
    listeners: HashMap<ActionId, ActionHandler>,
}

impl ActionListeners {
    /// Create a new empty listener set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for a specific action type.
    ///
    /// The handler receives the action (as `&dyn Any`) and returns `true` if handled.
    pub fn on_action<A: super::Action>(&mut self, mut handler: impl FnMut(&A) -> bool + 'static) {
        let action_id = std::any::TypeId::of::<A>();
        self.listeners.insert(
            action_id,
            Box::new(move |action: &dyn Any| {
                if let Some(typed) = action.downcast_ref::<A>() {
                    handler(typed)
                } else {
                    false
                }
            }),
        );
    }

    /// Try to handle an action.
    ///
    /// Returns `true` if the action was handled by a registered listener.
    pub fn handle(&mut self, action: &dyn AnyAction) -> bool {
        let action_id = action.action_id();
        if let Some(handler) = self.listeners.get_mut(&action_id) {
            handler(action.as_any())
        } else {
            false
        }
    }

    /// Check if there's a listener for the given action type.
    pub fn has_listener(&self, action_id: ActionId) -> bool {
        self.listeners.contains_key(&action_id)
    }

    /// Remove all listeners.
    pub fn clear(&mut self) {
        self.listeners.clear();
    }

    /// Get the number of registered listeners.
    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    /// Check if there are no listeners.
    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

/// Result of dispatching an action.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// Whether the action was handled.
    pub handled: bool,
    /// Component ID that handled the action (if any).
    pub handler_id: Option<u64>,
}

impl DispatchResult {
    /// Create a result indicating the action was handled.
    pub fn handled(component_id: u64) -> Self {
        Self {
            handled: true,
            handler_id: Some(component_id),
        }
    }

    /// Create a result indicating the action was not handled.
    pub fn not_handled() -> Self {
        Self {
            handled: false,
            handler_id: None,
        }
    }
}

/// A pending action waiting to be dispatched.
#[derive(Debug)]
pub struct PendingAction {
    /// The action to dispatch.
    pub action: Box<dyn AnyAction>,
    /// Target component ID (if any).
    pub target: Option<u64>,
}

impl PendingAction {
    /// Create a new pending action.
    pub fn new(action: Box<dyn AnyAction>) -> Self {
        Self {
            action,
            target: None,
        }
    }

    /// Create a pending action targeted at a specific component.
    pub fn targeted(action: Box<dyn AnyAction>, target: u64) -> Self {
        Self {
            action,
            target: Some(target),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, NoAction};

    #[derive(Debug, Clone, Default)]
    struct TestAction {
        value: i32,
    }

    impl Action for TestAction {
        fn name() -> &'static str {
            "test::TestAction"
        }
        fn boxed_clone(&self) -> Box<dyn AnyAction> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_action_listeners_new() {
        let listeners = ActionListeners::new();
        assert!(listeners.is_empty());
    }

    #[test]
    fn test_on_action_and_handle() {
        let mut listeners = ActionListeners::new();
        let mut received_value = 0;

        listeners.on_action::<TestAction>(move |action| {
            received_value = action.value;
            true
        });

        assert!(!listeners.is_empty());
        assert_eq!(listeners.len(), 1);

        let action = TestAction { value: 42 };
        let handled = listeners.handle(&action);
        assert!(handled);
    }

    #[test]
    fn test_handle_wrong_action_type() {
        let mut listeners = ActionListeners::new();

        listeners.on_action::<TestAction>(|_| true);

        // NoAction is not TestAction
        let action = NoAction;
        let handled = listeners.handle(&action);
        assert!(!handled);
    }

    #[test]
    fn test_has_listener() {
        let mut listeners = ActionListeners::new();

        let test_id = std::any::TypeId::of::<TestAction>();
        let no_action_id = std::any::TypeId::of::<NoAction>();

        assert!(!listeners.has_listener(test_id));

        listeners.on_action::<TestAction>(|_| true);

        assert!(listeners.has_listener(test_id));
        assert!(!listeners.has_listener(no_action_id));
    }

    #[test]
    fn test_dispatch_result() {
        let handled = DispatchResult::handled(42);
        assert!(handled.handled);
        assert_eq!(handled.handler_id, Some(42));

        let not_handled = DispatchResult::not_handled();
        assert!(!not_handled.handled);
        assert_eq!(not_handled.handler_id, None);
    }

    #[test]
    fn test_pending_action() {
        let action = TestAction { value: 42 };
        let pending = PendingAction::new(Box::new(action));
        assert!(pending.target.is_none());

        let action = TestAction { value: 99 };
        let targeted = PendingAction::targeted(Box::new(action), 123);
        assert_eq!(targeted.target, Some(123));
    }
}
