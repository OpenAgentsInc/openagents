//! Checkpoint restore control component.

use maud::{Markup, html};
use crate::acp::atoms::checkpoint_badge;

/// Checkpoint restore state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreState {
    /// Ready to restore
    Ready,
    /// Confirmation dialog shown
    Confirming,
    /// Restore in progress
    Restoring,
}

/// Checkpoint restore control with badge and action button.
pub struct CheckpointRestore {
    sha: String,
    state: RestoreState,
    entry_id: String,
}

impl CheckpointRestore {
    /// Create a new checkpoint restore control.
    pub fn new(sha: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            sha: sha.into(),
            state: RestoreState::Ready,
            entry_id: entry_id.into(),
        }
    }

    /// Set the restore state.
    pub fn state(mut self, state: RestoreState) -> Self {
        self.state = state;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class="flex items-center gap-2" {
                // Checkpoint badge
                (checkpoint_badge(&self.sha))

                // Action based on state
                @match self.state {
                    RestoreState::Ready => {
                        button
                            type="button"
                            class="px-2 py-1 text-xs bg-secondary border border-border hover:bg-accent"
                            data-restore-checkpoint=(self.entry_id)
                            data-sha=(self.sha)
                        {
                            "Restore"
                        }
                    }
                    RestoreState::Confirming => {
                        div class="flex gap-1" {
                            button
                                type="button"
                                class="px-2 py-1 text-xs bg-destructive text-foreground"
                                data-confirm-restore=(self.entry_id)
                            {
                                "Confirm"
                            }
                            button
                                type="button"
                                class="px-2 py-1 text-xs bg-secondary border border-border"
                                data-cancel-restore=(self.entry_id)
                            {
                                "Cancel"
                            }
                        }
                    }
                    RestoreState::Restoring => {
                        span class="text-xs text-muted-foreground animate-pulse" {
                            "Restoring..."
                        }
                    }
                }
            }
        }
    }
}
