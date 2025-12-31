//! Macros for defining actions.
//!
//! This module provides the `actions!` macro for concisely defining multiple
//! related actions.

/// Define simple action types in a namespace.
///
/// This macro generates unit structs that implement the `Action` trait.
/// Each action is namespaced with the provided identifier.
///
/// # Basic Usage
///
/// ```ignore
/// actions!(editor, [
///     Save,
///     Undo,
///     Redo,
///     Copy,
///     Paste,
/// ]);
///
/// // Creates:
/// // - editor::Save
/// // - editor::Undo
/// // - editor::Redo
/// // - editor::Copy
/// // - editor::Paste
/// ```
///
/// # With Fields
///
/// ```ignore
/// actions!(navigation, [
///     GoToLine { line: usize },
///     Search { query: String, case_sensitive: bool },
/// ]);
///
/// // Creates structs with the given fields:
/// // - navigation::GoToLine { line: usize }
/// // - navigation::Search { query: String, case_sensitive: bool }
/// ```
///
/// # Generated Implementation
///
/// For each action, the macro generates:
/// - A struct (unit struct or with fields)
/// - `Debug`, `Clone`, `Default` derives
/// - Implementation of the `Action` trait
/// - A `name()` that returns `"namespace::ActionName"`
#[macro_export]
macro_rules! actions {
    // Unit struct actions (no fields)
    ($namespace:ident, [ $($action:ident),* $(,)? ]) => {
        $(
            #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
            pub struct $action;

            impl $crate::Action for $action {
                fn name() -> &'static str {
                    concat!(stringify!($namespace), "::", stringify!($action))
                }

                fn boxed_clone(&self) -> Box<dyn $crate::AnyAction> {
                    Box::new(*self)
                }
            }
        )*
    };
}

/// Define a single action with optional fields.
///
/// This is a helper macro for defining individual actions with more control.
///
/// # Unit Action
///
/// ```ignore
/// action!(editor::Save);
/// // Creates editor::Save unit struct
/// ```
///
/// # Action with Fields
///
/// ```ignore
/// action!(editor::GoToLine {
///     line: usize,
/// });
/// // Creates editor::GoToLine { line: usize }
/// ```
#[macro_export]
macro_rules! action {
    // Unit action
    ($namespace:ident :: $action:ident) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $action;

        impl $crate::Action for $action {
            fn name() -> &'static str {
                concat!(stringify!($namespace), "::", stringify!($action))
            }

            fn boxed_clone(&self) -> Box<dyn $crate::AnyAction> {
                Box::new(*self)
            }
        }
    };

    // Action with fields
    ($namespace:ident :: $action:ident { $($field:ident : $type:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $action {
            $(pub $field: $type),*
        }

        impl $crate::Action for $action {
            fn name() -> &'static str {
                concat!(stringify!($namespace), "::", stringify!($action))
            }

            fn boxed_clone(&self) -> Box<dyn $crate::AnyAction> {
                Box::new(self.clone())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::action::{Action, AnyAction};

    // Test the actions! macro
    mod test_namespace {
        actions!(test_namespace, [ActionOne, ActionTwo, ActionThree,]);
    }

    #[test]
    fn test_actions_macro_creates_unit_structs() {
        let _a1 = test_namespace::ActionOne;
        let _a2 = test_namespace::ActionTwo;
        let _a3 = test_namespace::ActionThree;
    }

    #[test]
    fn test_actions_macro_names() {
        assert_eq!(
            <test_namespace::ActionOne as Action>::name(),
            "test_namespace::ActionOne"
        );
        assert_eq!(
            <test_namespace::ActionTwo as Action>::name(),
            "test_namespace::ActionTwo"
        );
    }

    #[test]
    fn test_actions_macro_boxed_clone() {
        let action = test_namespace::ActionOne;
        let boxed = Action::boxed_clone(&action);
        assert_eq!(boxed.name(), "test_namespace::ActionOne");
    }

    #[test]
    fn test_actions_macro_default() {
        let _a1: test_namespace::ActionOne = Default::default();
    }

    #[test]
    fn test_actions_macro_copy() {
        let a1 = test_namespace::ActionOne;
        let a2 = a1; // Copy
        assert_eq!(a1, a2);
    }

    // Test the action! macro
    mod single_action {
        action!(single::UnitAction);
    }

    #[test]
    fn test_action_macro_unit() {
        let _action = single_action::UnitAction;
        assert_eq!(
            <single_action::UnitAction as Action>::name(),
            "single::UnitAction"
        );
    }

    mod field_action {
        action!(field::GoToLine { line: usize });
    }

    #[test]
    fn test_action_macro_with_fields() {
        let action = field_action::GoToLine { line: 42 };
        assert_eq!(action.line, 42);
        assert_eq!(
            <field_action::GoToLine as Action>::name(),
            "field::GoToLine"
        );

        let boxed = Action::boxed_clone(&action);
        assert_eq!(boxed.name(), "field::GoToLine");
    }
}
