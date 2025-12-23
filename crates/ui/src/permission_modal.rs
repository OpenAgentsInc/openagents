//! Permission request modal component for ACP sessions

use maud::{html, Markup, PreEscaped};

/// Permission request modal component
///
/// Renders a modal dialog for requesting user permission to execute a tool.
/// Integrates with HTMX for real-time updates and form submission.
pub struct PermissionModal {
    request_id: String,
    session_id: String,
    tool_name: String,
    description: String,
    input: serde_json::Value,
    options: Vec<PermissionOption>,
}

/// Permission option for user selection
pub struct PermissionOption {
    option_id: String,
    label: String,
    kind: PermissionOptionKind,
    is_persistent: bool,
}

/// Permission option kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

impl PermissionModal {
    /// Create a new permission modal
    pub fn new(
        request_id: String,
        session_id: String,
        tool_name: String,
        description: String,
        input: serde_json::Value,
        options: Vec<PermissionOption>,
    ) -> Self {
        Self {
            request_id,
            session_id,
            tool_name,
            description,
            input,
            options,
        }
    }

    /// Render the permission modal
    pub fn render(&self) -> Markup {
        html! {
            div
                id=(format!("permission-modal-{}", self.request_id))
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
                role="dialog"
                aria-modal="true"
            {
                div class="bg-card border border-border p-6 max-w-2xl w-full max-h-[80vh] overflow-y-auto" {
                    // Header
                    div class="mb-4" {
                        h2 class="text-xl font-bold text-foreground mb-2" {
                            "Permission Required"
                        }
                        div class="text-sm text-muted-foreground" {
                            "Session: " (self.session_id)
                        }
                    }

                    // Tool information
                    div class="mb-6 p-4 bg-muted border border-border" {
                        div class="mb-2" {
                            span class="font-bold text-foreground" { "Tool: " }
                            span class="font-mono text-cyan" { (&self.tool_name) }
                        }
                        div class="mb-2" {
                            span class="font-bold text-foreground" { "Description: " }
                            span class="text-foreground" { (&self.description) }
                        }
                        @if !self.input.is_null() && self.input != serde_json::json!({}) {
                            div class="mt-4" {
                                div class="font-bold text-foreground mb-2" { "Parameters:" }
                                pre class="text-sm font-mono text-muted-foreground overflow-x-auto p-2 bg-background border border-border" {
                                    (PreEscaped(
                                        serde_json::to_string_pretty(&self.input)
                                            .unwrap_or_else(|_| "{}".to_string())
                                    ))
                                }
                            }
                        }
                    }

                    // Options
                    form
                        hx-post=(format!("/api/acp/permissions/{}/respond", self.request_id))
                        hx-swap="outerHTML"
                        hx-target=(format!("#permission-modal-{}", self.request_id))
                    {
                        input type="hidden" name="request_id" value=(&self.request_id);

                        div class="space-y-2 mb-6" {
                            @for option in &self.options {
                                label class="flex items-center p-3 border border-border hover:bg-accent cursor-pointer" {
                                    input
                                        type="radio"
                                        name="selected_option_id"
                                        value=(&option.option_id)
                                        required
                                        class="mr-3"
                                    ;
                                    div class="flex-1" {
                                        div class="font-bold text-foreground" {
                                            (&option.label)
                                        }
                                        @if option.is_persistent {
                                            div class="text-xs text-muted-foreground mt-1" {
                                                "This will create a permanent rule"
                                            }
                                        }
                                    }
                                    (Self::render_option_badge(option.kind))
                                }
                            }
                        }

                        // Persistence toggle (for non-persistent options)
                        @if self.options.iter().any(|o| !o.is_persistent) {
                            label class="flex items-center mb-6 p-2 bg-muted" {
                                input
                                    type="checkbox"
                                    name="make_persistent"
                                    value="true"
                                    class="mr-2"
                                ;
                                span class="text-sm text-foreground" {
                                    "Remember this choice for similar requests"
                                }
                            }
                        }

                        // Actions
                        div class="flex justify-end gap-3" {
                            button
                                type="submit"
                                class="px-4 py-2 bg-primary text-primary-foreground font-bold hover:bg-primary/90"
                            {
                                "Submit"
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render an option badge based on its kind
    fn render_option_badge(kind: PermissionOptionKind) -> Markup {
        let (color, text) = match kind {
            PermissionOptionKind::AllowOnce => ("text-green", "Once"),
            PermissionOptionKind::AllowAlways => ("text-green", "Always"),
            PermissionOptionKind::RejectOnce => ("text-red", "Once"),
            PermissionOptionKind::RejectAlways => ("text-red", "Always"),
        };

        html! {
            span class=(format!("text-xs font-mono px-2 py-1 border border-border {}", color)) {
                (text)
            }
        }
    }

    /// Render the response confirmation (replaces modal after submission)
    pub fn render_response_confirmation(request_id: &str, success: bool) -> Markup {
        if success {
            html! {
                div
                    id=(format!("permission-modal-{}", request_id))
                    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
                    hx-swap-oob="true"
                {
                    div class="bg-card border border-green p-6 max-w-md" {
                        div class="text-center" {
                            div class="text-green text-4xl mb-2" { "✓" }
                            div class="text-xl font-bold text-foreground mb-2" {
                                "Permission Submitted"
                            }
                            div class="text-muted-foreground mb-4" {
                                "Your response has been recorded"
                            }
                            button
                                class="px-4 py-2 bg-secondary text-secondary-foreground"
                                onclick=(format!("document.getElementById('permission-modal-{}').remove()", request_id))
                            {
                                "Close"
                            }
                        }
                    }
                }
            }
        } else {
            html! {
                div
                    id=(format!("permission-modal-{}", request_id))
                    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
                    hx-swap-oob="true"
                {
                    div class="bg-card border border-red p-6 max-w-md" {
                        div class="text-center" {
                            div class="text-red text-4xl mb-2" { "✗" }
                            div class="text-xl font-bold text-foreground mb-2" {
                                "Error"
                            }
                            div class="text-muted-foreground mb-4" {
                                "Failed to submit permission response"
                            }
                            button
                                class="px-4 py-2 bg-secondary text-secondary-foreground"
                                onclick=(format!("document.getElementById('permission-modal-{}').remove()", request_id))
                            {
                                "Close"
                            }
                        }
                    }
                }
            }
        }
    }
}

impl PermissionOption {
    /// Create a new permission option
    pub fn new(
        option_id: String,
        label: String,
        kind: PermissionOptionKind,
        is_persistent: bool,
    ) -> Self {
        Self {
            option_id,
            label,
            kind,
            is_persistent,
        }
    }
}
