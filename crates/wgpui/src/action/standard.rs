//! Standard UI actions.
//!
//! These actions represent common UI operations that can be bound to keys.

use super::{Action, AnyAction};

// ============================================================================
// Navigation Actions
// ============================================================================

/// Move selection/cursor up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveUp;

impl Action for MoveUp {
    fn name() -> &'static str {
        "MoveUp"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move selection/cursor down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveDown;

impl Action for MoveDown {
    fn name() -> &'static str {
        "MoveDown"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move selection/cursor left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveLeft;

impl Action for MoveLeft {
    fn name() -> &'static str {
        "MoveLeft"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move selection/cursor right.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveRight;

impl Action for MoveRight {
    fn name() -> &'static str {
        "MoveRight"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move to start (home).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveToStart;

impl Action for MoveToStart {
    fn name() -> &'static str {
        "MoveToStart"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move to end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveToEnd;

impl Action for MoveToEnd {
    fn name() -> &'static str {
        "MoveToEnd"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// Editing Actions
// ============================================================================

/// Cancel current operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cancel;

impl Action for Cancel {
    fn name() -> &'static str {
        "Cancel"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Confirm/submit current operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Confirm;

impl Action for Confirm {
    fn name() -> &'static str {
        "Confirm"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Delete forward (delete key).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Delete;

impl Action for Delete {
    fn name() -> &'static str {
        "Delete"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Delete backward (backspace).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Backspace;

impl Action for Backspace {
    fn name() -> &'static str {
        "Backspace"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Select all content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectAll;

impl Action for SelectAll {
    fn name() -> &'static str {
        "SelectAll"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// Clipboard Actions
// ============================================================================

/// Copy selection to clipboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Copy;

impl Action for Copy {
    fn name() -> &'static str {
        "Copy"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Cut selection to clipboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cut;

impl Action for Cut {
    fn name() -> &'static str {
        "Cut"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Paste from clipboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Paste;

impl Action for Paste {
    fn name() -> &'static str {
        "Paste"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// Undo/Redo Actions
// ============================================================================

/// Undo last action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Undo;

impl Action for Undo {
    fn name() -> &'static str {
        "Undo"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Redo last undone action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Redo;

impl Action for Redo {
    fn name() -> &'static str {
        "Redo"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// Focus Actions
// ============================================================================

/// Move focus to next focusable element.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusNext;

impl Action for FocusNext {
    fn name() -> &'static str {
        "FocusNext"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Move focus to previous focusable element.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusPrevious;

impl Action for FocusPrevious {
    fn name() -> &'static str {
        "FocusPrevious"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// UI Actions
// ============================================================================

/// Toggle command palette visibility.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToggleCommandPalette;

impl Action for ToggleCommandPalette {
    fn name() -> &'static str {
        "ToggleCommandPalette"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Close current panel/modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Close;

impl Action for Close {
    fn name() -> &'static str {
        "Close"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Refresh/reload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Refresh;

impl Action for Refresh {
    fn name() -> &'static str {
        "Refresh"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

// ============================================================================
// File Actions
// ============================================================================

/// Save current file/document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Save;

impl Action for Save {
    fn name() -> &'static str {
        "Save"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Open file picker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Open;

impl Action for Open {
    fn name() -> &'static str {
        "Open"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

/// Create new file/document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct New;

impl Action for New {
    fn name() -> &'static str {
        "New"
    }
    fn boxed_clone(&self) -> Box<dyn AnyAction> {
        Box::new(*self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_names() {
        assert_eq!(<MoveUp as Action>::name(), "MoveUp");
        assert_eq!(<Cancel as Action>::name(), "Cancel");
        assert_eq!(<Copy as Action>::name(), "Copy");
        assert_eq!(<Save as Action>::name(), "Save");
    }

    #[test]
    fn test_boxed_clone() {
        let action = MoveUp;
        let boxed = Action::boxed_clone(&action);
        assert_eq!(boxed.name(), "MoveUp");
    }

    #[test]
    fn test_default() {
        let _action: MoveUp = Default::default();
        let _action: Cancel = Default::default();
        let _action: Copy = Default::default();
    }
}
