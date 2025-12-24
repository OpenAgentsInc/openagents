//! Message editor section component.

use maud::{Markup, html};
use crate::acp::styles::ACP_EDITOR_CLASS;

/// Message editor for user input.
pub struct MessageEditor {
    session_id: String,
    placeholder: String,
    value: String,
    disabled: bool,
}

impl MessageEditor {
    /// Create a new message editor.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            placeholder: "Type a message...".to_string(),
            value: String::new(),
            disabled: false,
        }
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set the initial value.
    pub fn value(mut self, text: impl Into<String>) -> Self {
        self.value = text.into();
        self
    }

    /// Disable the editor.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class=(ACP_EDITOR_CLASS) {
                // Input area
                div class="flex gap-2" {
                    // Textarea
                    textarea
                        class={
                            "flex-1 px-3 py-2 text-sm bg-background border border-border "
                            "focus:outline-none focus:border-primary resize-none "
                            @if self.disabled { "opacity-50 cursor-not-allowed" }
                        }
                        rows="3"
                        placeholder=(self.placeholder)
                        disabled[self.disabled]
                        data-message-input=(self.session_id)
                    {
                        (self.value)
                    }

                    // Submit button
                    button
                        type="button"
                        class={
                            "px-4 py-2 bg-primary text-primary-foreground text-sm font-medium "
                            "hover:bg-primary/90 self-end "
                            @if self.disabled { "opacity-50 cursor-not-allowed" }
                        }
                        disabled[self.disabled]
                        data-send-message=(self.session_id)
                    {
                        "Send"
                    }
                }

                // Hints
                div class="mt-2 flex items-center gap-4 text-xs text-muted-foreground" {
                    span {
                        kbd class="px-1 py-0.5 bg-secondary border border-border font-mono text-[10px]" {
                            "Enter"
                        }
                        " to send"
                    }
                    span {
                        kbd class="px-1 py-0.5 bg-secondary border border-border font-mono text-[10px]" {
                            "Shift+Enter"
                        }
                        " for new line"
                    }
                    span {
                        "Use "
                        code class="font-mono" { "@" }
                        " to mention files"
                    }
                }
            }
        }
    }
}
