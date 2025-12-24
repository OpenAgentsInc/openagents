//! Model selector dropdown.

use maud::{Markup, html};
use crate::acp::atoms::model_badge;

/// Model info for selection.
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// Model selector with current model and available options.
pub struct ModelSelector {
    current: String,
    available: Vec<ModelInfo>,
    session_id: String,
}

impl ModelSelector {
    /// Create a new model selector.
    pub fn new(current: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            current: current.into(),
            available: Vec::new(),
            session_id: session_id.into(),
        }
    }

    /// Set available models.
    pub fn available(mut self, models: Vec<ModelInfo>) -> Self {
        self.available = models;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            details class="relative group" {
                summary class="cursor-pointer list-none" {
                    (model_badge(&self.current, true))
                }

                // Dropdown menu
                div class="absolute top-full left-0 mt-1 z-50 bg-popover border border-border min-w-[200px] max-h-[300px] overflow-y-auto" {
                    @if self.available.is_empty() {
                        div class="px-3 py-2 text-sm text-muted-foreground" {
                            "No other models available"
                        }
                    } @else {
                        @for model in &self.available {
                            button
                                type="button"
                                class={
                                    "block w-full text-left px-3 py-2 text-sm hover:bg-accent "
                                    @if model.id == self.current { "bg-accent" }
                                }
                                data-session-id=(self.session_id)
                                data-model-id=(model.id)
                            {
                                div class="font-medium" { (model.name) }
                                div class="text-xs text-muted-foreground font-mono" { (model.id) }
                            }
                        }
                    }
                }
            }
        }
    }
}
