//! Permission action bar component.

use maud::{Markup, html};
use crate::acp::atoms::{permission_button, keybinding_hint, PermissionKind};
use crate::acp::styles::ACP_PERMISSION_BAR_CLASS;

/// Permission request action bar.
pub struct PermissionBar {
    show_keybindings: bool,
    option_ids: Option<PermissionOptionIds>,
}

/// Optional IDs for each permission option.
pub struct PermissionOptionIds {
    pub allow_once: String,
    pub allow_always: String,
    pub reject_once: String,
    pub reject_always: String,
}

impl PermissionBar {
    /// Create a new permission bar.
    pub fn new() -> Self {
        Self {
            show_keybindings: true,
            option_ids: None,
        }
    }

    /// Hide keyboard shortcut hints.
    pub fn hide_keybindings(mut self) -> Self {
        self.show_keybindings = false;
        self
    }

    /// Set option IDs for the buttons.
    pub fn option_ids(mut self, ids: PermissionOptionIds) -> Self {
        self.option_ids = Some(ids);
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let (allow_once_id, allow_always_id, reject_once_id, reject_always_id) =
            if let Some(ids) = &self.option_ids {
                (
                    Some(ids.allow_once.as_str()),
                    Some(ids.allow_always.as_str()),
                    Some(ids.reject_once.as_str()),
                    Some(ids.reject_always.as_str()),
                )
            } else {
                (None, None, None, None)
            };

        html! {
            div class=(ACP_PERMISSION_BAR_CLASS) {
                // Allow buttons
                div class="flex gap-2" {
                    (permission_button(PermissionKind::AllowOnce, allow_once_id))
                    (permission_button(PermissionKind::AllowAlways, allow_always_id))
                }

                // Separator
                div class="flex-1" {}

                // Reject buttons
                div class="flex gap-2" {
                    (permission_button(PermissionKind::RejectOnce, reject_once_id))
                    (permission_button(PermissionKind::RejectAlways, reject_always_id))
                }
            }

            // Keybinding hints
            @if self.show_keybindings {
                div class="px-3 py-1 flex gap-4 text-xs text-muted-foreground border-t border-border" {
                    (keybinding_hint("y", "allow"))
                    (keybinding_hint("Y", "always allow"))
                    (keybinding_hint("n", "reject"))
                    (keybinding_hint("N", "always reject"))
                }
            }
        }
    }
}

impl Default for PermissionBar {
    fn default() -> Self {
        Self::new()
    }
}
