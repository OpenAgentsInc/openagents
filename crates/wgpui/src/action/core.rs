//! Core action types and traits.

use std::any::{Any, TypeId};
use std::fmt::Debug;

/// Unique identifier for action types.
pub type ActionId = TypeId;

/// Trait for all dispatchable actions.
///
/// Actions are value types that represent user intents (e.g., Save, Copy, MoveUp).
/// They carry any necessary parameters and can be dispatched through the UI tree.
///
/// # Example
///
/// ```ignore
/// #[derive(Debug, Clone, Default)]
/// struct Save;
///
/// impl Action for Save {
///     fn name() -> &'static str { "editor::Save" }
///     fn boxed_clone(&self) -> Box<dyn AnyAction> { Box::new(self.clone()) }
/// }
/// ```
pub trait Action: Any + Debug + Clone + Send + Sync + 'static {
    /// Human-readable name for debugging and keybinding display.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Unique type identifier.
    fn action_id(&self) -> ActionId {
        TypeId::of::<Self>()
    }

    /// Clone into boxed trait object.
    fn boxed_clone(&self) -> Box<dyn AnyAction>;
}

/// Type-erased action for storage in registries and dispatch.
pub trait AnyAction: Any + Debug + Send + Sync {
    /// Get the unique type identifier for this action.
    fn action_id(&self) -> ActionId;

    /// Get the human-readable name.
    fn name(&self) -> &'static str;

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Clone into a new boxed action.
    fn boxed_clone(&self) -> Box<dyn AnyAction>;
}

impl<A: Action> AnyAction for A {
    fn action_id(&self) -> ActionId {
        TypeId::of::<A>()
    }

    fn name(&self) -> &'static str {
        A::name()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(self.clone())
    }
}

/// Placeholder action that does nothing.
///
/// Useful for testing or as a sentinel value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoAction;

impl Action for NoAction {
    fn name() -> &'static str {
        "NoAction"
    }

    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
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

    #[test]
    fn test_action_id() {
        let action = TestAction;
        assert_eq!(Action::action_id(&action), TypeId::of::<TestAction>());
    }

    #[test]
    fn test_boxed_clone() {
        let action = TestAction;
        let boxed = Action::boxed_clone(&action);
        assert_eq!(boxed.name(), "test::TestAction");
    }

    #[test]
    fn test_downcast() {
        let action: Box<dyn AnyAction> = Box::new(TestAction);
        let downcasted = action.as_any().downcast_ref::<TestAction>();
        assert!(downcasted.is_some());
    }

    #[test]
    fn test_no_action() {
        let action = NoAction;
        assert_eq!(<NoAction as Action>::name(), "NoAction");
        let boxed = Action::boxed_clone(&action);
        assert_eq!(boxed.name(), "NoAction");
    }
}
