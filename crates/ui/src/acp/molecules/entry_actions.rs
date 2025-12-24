//! Entry action buttons component.

use maud::{Markup, html};

/// Available actions for a thread entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    /// Copy entry content
    Copy,
    /// Regenerate from this point (user messages)
    Regenerate,
    /// Cancel editing
    CancelEdit,
}

/// Entry action buttons.
pub struct EntryActions {
    actions: Vec<EntryAction>,
    entry_id: String,
}

impl EntryActions {
    /// Create entry actions for a user message.
    pub fn for_user(entry_id: impl Into<String>) -> Self {
        Self {
            actions: vec![EntryAction::Copy, EntryAction::Regenerate],
            entry_id: entry_id.into(),
        }
    }

    /// Create entry actions for an assistant message.
    pub fn for_assistant(entry_id: impl Into<String>) -> Self {
        Self {
            actions: vec![EntryAction::Copy],
            entry_id: entry_id.into(),
        }
    }

    /// Create entry actions for editing mode.
    pub fn for_editing(entry_id: impl Into<String>) -> Self {
        Self {
            actions: vec![EntryAction::CancelEdit],
            entry_id: entry_id.into(),
        }
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class="flex gap-1" {
                @for action in &self.actions {
                    (render_action_button(*action, &self.entry_id))
                }
            }
        }
    }
}

fn render_action_button(action: EntryAction, entry_id: &str) -> Markup {
    let (icon, label, _data_attr) = match action {
        EntryAction::Copy => ("[c]", "Copy", "data-copy-entry"),
        EntryAction::Regenerate => ("[r]", "Regenerate", "data-regenerate-entry"),
        EntryAction::CancelEdit => ("[x]", "Cancel", "data-cancel-edit"),
    };

    let attr_value = entry_id;
    match action {
        EntryAction::Copy => html! {
            button
                type="button"
                class="p-1 text-xs text-muted-foreground hover:text-foreground hover:bg-secondary"
                title=(label)
                data-copy-entry=(attr_value)
            {
                (icon)
            }
        },
        EntryAction::Regenerate => html! {
            button
                type="button"
                class="p-1 text-xs text-muted-foreground hover:text-foreground hover:bg-secondary"
                title=(label)
                data-regenerate-entry=(attr_value)
            {
                (icon)
            }
        },
        EntryAction::CancelEdit => html! {
            button
                type="button"
                class="p-1 text-xs text-muted-foreground hover:text-foreground hover:bg-secondary"
                title=(label)
                data-cancel-edit=(attr_value)
            {
                (icon)
            }
        },
    }
}
