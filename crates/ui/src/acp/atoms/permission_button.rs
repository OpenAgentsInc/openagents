//! Permission action buttons for tool authorization.

use maud::{Markup, html};

/// Kind of permission action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKind {
    /// Allow this specific request
    AllowOnce,
    /// Always allow this type of request
    AllowAlways,
    /// Reject this specific request
    RejectOnce,
    /// Always reject this type of request
    RejectAlways,
}

impl PermissionKind {
    /// Button label text.
    pub fn label(&self) -> &'static str {
        match self {
            PermissionKind::AllowOnce => "Allow",
            PermissionKind::AllowAlways => "Always Allow",
            PermissionKind::RejectOnce => "Reject",
            PermissionKind::RejectAlways => "Always Reject",
        }
    }

    /// Keyboard shortcut for this action.
    pub fn keybinding(&self) -> &'static str {
        match self {
            PermissionKind::AllowOnce => "y",
            PermissionKind::AllowAlways => "Y",
            PermissionKind::RejectOnce => "n",
            PermissionKind::RejectAlways => "N",
        }
    }

    /// CSS classes for the button variant.
    fn button_class(&self) -> &'static str {
        match self {
            PermissionKind::AllowOnce => {
                "bg-green text-background hover:bg-green/90 px-3 py-1 text-xs font-medium"
            }
            PermissionKind::AllowAlways => {
                "bg-secondary text-foreground hover:bg-accent px-3 py-1 text-xs font-medium border border-border"
            }
            PermissionKind::RejectOnce => {
                "bg-secondary text-foreground hover:bg-accent px-3 py-1 text-xs font-medium border border-border"
            }
            PermissionKind::RejectAlways => {
                "bg-destructive text-foreground hover:bg-destructive/90 px-3 py-1 text-xs font-medium"
            }
        }
    }
}

/// Render a permission action button.
///
/// # Arguments
/// * `kind` - The type of permission action
/// * `option_id` - Optional ID to include in the button's data attribute
pub fn permission_button(kind: PermissionKind, option_id: Option<&str>) -> Markup {
    html! {
        button
            type="button"
            class=(kind.button_class())
            data-permission-kind=(format!("{:?}", kind))
            data-option-id=[option_id]
            title={ (kind.label()) " (" (kind.keybinding()) ")" }
        {
            (kind.label())
        }
    }
}
