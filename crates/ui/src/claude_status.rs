//! ClaudeStatus component.
//!
//! Shows Claude login/authentication status in a compact card.
//! Displays model, version, sessions, messages, today's tokens, and per-model usage.

use maud::{Markup, html};

/// Per-model usage for display
pub struct ModelUsageDisplay {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

/// Claude authentication status display.
#[derive(Default)]
pub struct ClaudeStatus {
    /// Whether loading
    pub loading: bool,
    /// Whether authenticated
    pub authenticated: bool,
    /// Current model
    pub model: Option<String>,
    /// Claude Code version
    pub version: Option<String>,
    /// Total sessions
    pub total_sessions: Option<u64>,
    /// Total messages
    pub total_messages: Option<u64>,
    /// Today's token count
    pub today_tokens: Option<u64>,
    /// Per-model usage
    pub model_usage: Vec<ModelUsageDisplay>,
}

impl ClaudeStatus {
    /// Create a loading status display.
    pub fn loading() -> Self {
        Self {
            loading: true,
            ..Default::default()
        }
    }

    /// Create a new status display with no data (not logged in).
    pub fn not_logged_in() -> Self {
        Self::default()
    }

    /// Create a new status display with account info.
    pub fn authenticated() -> Self {
        Self {
            authenticated: true,
            ..Default::default()
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn total_sessions(mut self, sessions: u64) -> Self {
        self.total_sessions = Some(sessions);
        self
    }

    pub fn total_messages(mut self, messages: u64) -> Self {
        self.total_messages = Some(messages);
        self
    }

    pub fn today_tokens(mut self, tokens: u64) -> Self {
        self.today_tokens = Some(tokens);
        self
    }

    pub fn add_model_usage(mut self, model: String, input: u64, output: u64, cache_read: u64, cache_create: u64) -> Self {
        self.model_usage.push(ModelUsageDisplay {
            model,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_create,
        });
        self
    }

    /// Render the component for positioning (call this for the full positioned version).
    /// Includes HTMX polling to refresh status.
    pub fn build_positioned(self) -> Markup {
        html! {
            div
                id="claude-status"
                style="position: fixed; bottom: 1rem; right: 1rem;"
                hx-get="/api/claude/status"
                hx-trigger="load, every 5s"
                hx-swap="innerHTML"
            {
                (self.build())
            }
        }
    }

    /// Render just the card (for embedding or storybook).
    pub fn build(self) -> Markup {
        html! {
            div
                class="claude-status-card"
                style="
                    background: #111;
                    border: 1px solid #333;
                    padding: 0.75rem 1rem;
                    font-family: 'Berkeley Mono', ui-monospace, monospace;
                    font-size: 0.65rem;
                    min-width: 220px;
                "
            {
                // Header row
                div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem;" {
                    // Status dot - yellow when loading, green when authed, red otherwise
                    span style={
                        "width: 6px; height: 6px; display: inline-block; "
                        @if self.loading { "background: #FFB800;" }
                        @else if self.authenticated { "background: #00A645;" }
                        @else { "background: #FF0000;" }
                    } {}
                    span style="color: #888; text-transform: uppercase; letter-spacing: 0.05em;" {
                        "CLAUDE"
                    }
                    @if let Some(ref ver) = self.version {
                        span style="color: #555;" { "v" (ver) }
                    }
                }

                @if self.loading {
                    div style="color: #888;" {
                        "Loading..."
                    }
                } @else if self.authenticated {
                    // Model
                    @if let Some(ref model) = self.model {
                        div style="color: #fafafa; margin-bottom: 0.35rem;" {
                            (format_model(model))
                        }
                    }

                    // Stats row
                    div style="display: flex; gap: 1rem; color: #666; margin-top: 0.5rem;" {
                        @if let Some(sessions) = self.total_sessions {
                            div {
                                span style="color: #888;" { (format_number(sessions)) }
                                span style="color: #555; margin-left: 0.25rem;" { "sessions" }
                            }
                        }
                        @if let Some(messages) = self.total_messages {
                            div {
                                span style="color: #888;" { (format_number(messages)) }
                                span style="color: #555; margin-left: 0.25rem;" { "msgs" }
                            }
                        }
                    }

                    // Today's tokens
                    @if let Some(tokens) = self.today_tokens {
                        div style="color: #555; margin-top: 0.35rem;" {
                            "Today: "
                            span style="color: #888;" { (format_number(tokens)) }
                            " tokens"
                        }
                    }

                    // Per-model usage
                    @if !self.model_usage.is_empty() {
                        div style="margin-top: 0.75rem; border-top: 1px solid #333; padding-top: 0.5rem;" {
                            div style="color: #555; margin-bottom: 0.35rem; text-transform: uppercase; letter-spacing: 0.05em;" {
                                "All-time usage"
                            }
                            @for usage in &self.model_usage {
                                div style="display: flex; justify-content: space-between; color: #666; margin-top: 0.25rem;" {
                                    span style="color: #888;" { (format_model(&usage.model)) }
                                    span style="color: #555;" {
                                        (format_tokens(usage.output_tokens))
                                        " out"
                                    }
                                }
                            }
                        }
                    }
                } @else {
                    div style="color: #666;" {
                        "Not authenticated"
                    }
                }
            }
        }
    }
}

/// Format model name to be shorter
fn format_model(model: &str) -> String {
    model
        .replace("claude-", "")
        .replace("-20251101", "")
        .replace("-20250929", "")
        .replace("-20250514", "")
        .replace("-20251001", "")
}

/// Format large numbers with K/M suffix
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format token counts with B/M/K suffix
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
