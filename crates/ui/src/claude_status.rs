//! ClaudeStatus component.
//!
//! Shows Claude login/authentication status in a compact card.
//! Displays model, version, sessions, messages, today's tokens, and per-model usage.
//! Now includes full usage stats from claude-agent-sdk:
//! - Context window usage (tokens used / total, percentage)
//! - Session cost breakdown
//! - Cache efficiency (read vs creation tokens)
//! - Web search request counts
//! - Duration and API latency

use maud::{Markup, html};

/// Per-model usage for display (matches claude-agent-sdk ModelUsage)
pub struct ModelUsageDisplay {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub web_search_requests: u64,
    pub cost_usd: f64,
    pub context_window: u64,
}

/// Session-level usage stats (from SDK result messages)
#[derive(Default)]
pub struct SessionUsage {
    /// Total input tokens
    pub input_tokens: u64,
    /// Total output tokens
    pub output_tokens: u64,
    /// Cache read tokens
    pub cache_read_tokens: u64,
    /// Cache creation tokens
    pub cache_creation_tokens: u64,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// API duration in milliseconds
    pub duration_api_ms: u64,
    /// Number of turns
    pub num_turns: u32,
}

/// Context window info
#[derive(Default)]
pub struct ContextWindow {
    /// Current tokens used
    pub current_tokens: u64,
    /// Context window size
    pub window_size: u64,
}

impl ContextWindow {
    pub fn percentage(&self) -> f64 {
        if self.window_size == 0 {
            0.0
        } else {
            (self.current_tokens as f64 / self.window_size as f64) * 100.0
        }
    }
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
    /// Current session usage
    pub session_usage: Option<SessionUsage>,
    /// Context window info
    pub context_window: Option<ContextWindow>,
    /// Total web searches this session
    pub web_searches: Option<u64>,
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

    pub fn add_model_usage(
        mut self,
        model: String,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_create: u64,
        web_searches: u64,
        cost_usd: f64,
        context_window: u64,
    ) -> Self {
        self.model_usage.push(ModelUsageDisplay {
            model,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_create,
            web_search_requests: web_searches,
            cost_usd,
            context_window,
        });
        self
    }

    pub fn session_usage(mut self, usage: SessionUsage) -> Self {
        self.session_usage = Some(usage);
        self
    }

    pub fn context_window(mut self, current: u64, size: u64) -> Self {
        self.context_window = Some(ContextWindow {
            current_tokens: current,
            window_size: size,
        });
        self
    }

    pub fn web_searches(mut self, count: u64) -> Self {
        self.web_searches = Some(count);
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

                    // Context window progress bar
                    @if let Some(ref ctx) = self.context_window {
                        div style="margin-top: 0.5rem;" {
                            div style="display: flex; justify-content: space-between; color: #555; margin-bottom: 0.2rem;" {
                                span { "Context" }
                                span { (format!("{:.0}%", ctx.percentage())) }
                            }
                            div style="height: 4px; background: #333; width: 100%;" {
                                div style={
                                    "height: 100%; transition: width 0.3s; "
                                    "width: " (format!("{:.1}%", ctx.percentage().min(100.0))) "; "
                                    "background: " (context_bar_color(ctx.percentage())) ";"
                                } {}
                            }
                            div style="display: flex; justify-content: space-between; color: #444; font-size: 0.55rem; margin-top: 0.15rem;" {
                                span { (format_tokens(ctx.current_tokens)) }
                                span { (format_tokens(ctx.window_size)) }
                            }
                        }
                    }

                    // Session usage (current session stats)
                    @if let Some(ref usage) = self.session_usage {
                        div style="margin-top: 0.75rem; border-top: 1px solid #333; padding-top: 0.5rem;" {
                            div style="color: #555; margin-bottom: 0.35rem; text-transform: uppercase; letter-spacing: 0.05em;" {
                                "Session"
                            }
                            // Cost and turns
                            div style="display: flex; justify-content: space-between; color: #666;" {
                                span style="color: #888;" {
                                    "$" (format!("{:.4}", usage.total_cost_usd))
                                }
                                span style="color: #555;" {
                                    (usage.num_turns) " turns"
                                }
                            }
                            // Tokens breakdown
                            div style="display: grid; grid-template-columns: 1fr 1fr; gap: 0.25rem; margin-top: 0.35rem; color: #555;" {
                                div {
                                    span style="color: #666;" { (format_tokens(usage.input_tokens)) }
                                    " in"
                                }
                                div {
                                    span style="color: #666;" { (format_tokens(usage.output_tokens)) }
                                    " out"
                                }
                                div {
                                    span style="color: #4a9;" { (format_tokens(usage.cache_read_tokens)) }
                                    " cached"
                                }
                                div {
                                    span style="color: #a94;" { (format_tokens(usage.cache_creation_tokens)) }
                                    " written"
                                }
                            }
                            // Duration
                            div style="display: flex; justify-content: space-between; color: #444; margin-top: 0.35rem; font-size: 0.55rem;" {
                                span { (format_duration(usage.duration_ms)) " total" }
                                span { (format_duration(usage.duration_api_ms)) " api" }
                            }
                        }
                    }

                    // Web searches
                    @if let Some(searches) = self.web_searches {
                        @if searches > 0 {
                            div style="color: #555; margin-top: 0.35rem;" {
                                span style="color: #69b;" { (searches) }
                                " web searches"
                            }
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
                                "Model usage"
                            }
                            @for usage in &self.model_usage {
                                div style="margin-bottom: 0.5rem;" {
                                    div style="display: flex; justify-content: space-between; color: #666;" {
                                        span style="color: #888;" { (format_model(&usage.model)) }
                                        span style={ "color: " (cost_color(usage.cost_usd)) ";" } {
                                            "$" (format!("{:.2}", usage.cost_usd))
                                        }
                                    }
                                    div style="display: flex; gap: 0.5rem; color: #444; font-size: 0.55rem; margin-top: 0.15rem;" {
                                        span { (format_tokens(usage.input_tokens)) " in" }
                                        span { (format_tokens(usage.output_tokens)) " out" }
                                        @if usage.cache_read_tokens > 0 {
                                            span style="color: #4a9;" { (format_tokens(usage.cache_read_tokens)) " hit" }
                                        }
                                        @if usage.web_search_requests > 0 {
                                            span style="color: #69b;" { (usage.web_search_requests) " web" }
                                        }
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

/// Get color for context bar based on percentage
fn context_bar_color(pct: f64) -> &'static str {
    if pct < 50.0 {
        "#00A645" // green
    } else if pct < 75.0 {
        "#FFB800" // yellow
    } else if pct < 90.0 {
        "#FF8800" // orange
    } else {
        "#FF0000" // red
    }
}

/// Get color for cost display
fn cost_color(cost: f64) -> &'static str {
    if cost < 0.10 {
        "#4a9" // green
    } else if cost < 1.0 {
        "#aa4" // yellow
    } else if cost < 10.0 {
        "#a94" // orange
    } else {
        "#a44" // red
    }
}

/// Format duration from milliseconds
fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else if ms < 3_600_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m{}s", mins, secs)
    } else {
        let hours = ms / 3_600_000;
        let mins = (ms % 3_600_000) / 60_000;
        format!("{}h{}m", hours, mins)
    }
}
