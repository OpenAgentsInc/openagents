//! BlackBox atom components for session log display.

use maud::{Markup, html};

// =============================================================================
// STATUS DOT
// =============================================================================

/// Status indicator dot with color variants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusState {
    Success,
    Running,
    Pending,
    Error,
    Skipped,
}

impl StatusState {
    fn class(&self) -> &'static str {
        match self {
            StatusState::Success => "text-green",
            StatusState::Running => "text-blue",
            StatusState::Pending => "text-yellow",
            StatusState::Error => "text-red",
            StatusState::Skipped => "text-muted-foreground opacity-60",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            StatusState::Success => "success",
            StatusState::Running => "running",
            StatusState::Pending => "pending",
            StatusState::Error => "error",
            StatusState::Skipped => "skipped",
        }
    }

    fn dot_char(&self) -> &'static str {
        match self {
            StatusState::Skipped => "\u{25CB}", // ○
            _ => "\u{25CF}",                    // ●
        }
    }
}

pub fn status_dot(state: StatusState) -> Markup {
    html! {
        span title=(state.label()) class={ "text-xs leading-none " (state.class()) } {
            (state.dot_char())
        }
    }
}

// =============================================================================
// LINE TYPE LABEL
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LineType {
    User,
    Agent,
    Tool,
    Observation,
    Skill,
    Plan,
    Mode,
    Recall,
    Subagent,
    Mcp,
    Question,
    Comment,
    Lifecycle,
    Phase,
}

impl LineType {
    fn label(&self) -> &'static str {
        match self {
            LineType::User => "USER",
            LineType::Agent => "AGENT",
            LineType::Tool => "TOOL",
            LineType::Observation => "OBSERVATION",
            LineType::Skill => "SKILL",
            LineType::Plan => "PLAN",
            LineType::Mode => "MODE",
            LineType::Recall => "RECALL",
            LineType::Subagent => "SUBAGENT",
            LineType::Mcp => "MCP",
            LineType::Question => "QUESTION",
            LineType::Comment => "#",
            LineType::Lifecycle => "@",
            LineType::Phase => "\u{25D0}", // ◐
        }
    }
}

pub fn line_type_label(line_type: LineType) -> Markup {
    html! {
        span class="text-xs uppercase tracking-widest text-muted-foreground" {
            (line_type.label())
        }
    }
}

// =============================================================================
// STEP BADGE
// =============================================================================

pub fn step_badge(step: u32) -> Markup {
    html! {
        span
            title={ "Step " (step) }
            class="inline-flex items-center px-1.5 py-0.5 text-xs bg-secondary text-muted-foreground cursor-pointer"
        {
            "[" (step) "]"
        }
    }
}

// =============================================================================
// TIMESTAMP BADGE
// =============================================================================

pub fn timestamp_badge_elapsed(hours: u8, minutes: u8, seconds: u8) -> Markup {
    html! {
        span
            title="Elapsed time"
            class="text-xs text-muted-foreground opacity-60 cursor-pointer tabular-nums"
        {
            (format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
        }
    }
}

pub fn timestamp_badge_wall(iso_short: &str) -> Markup {
    html! {
        span
            title="Wall clock time"
            class="text-xs text-muted-foreground opacity-60 cursor-pointer tabular-nums"
        {
            (iso_short)
        }
    }
}

// =============================================================================
// CALL ID BADGE
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    Tool,
    Mcp,
    Subagent,
}

impl CallType {
    fn class(&self) -> &'static str {
        match self {
            CallType::Tool => "text-yellow",
            CallType::Mcp => "text-cyan",
            CallType::Subagent => "text-red",
        }
    }
}

pub fn call_id_badge(call_id: &str, call_type: CallType) -> Markup {
    html! {
        span
            title={ "Call ID: " (call_id) }
            class={ "text-xs cursor-pointer " (call_type.class()) }
        {
            (call_id)
        }
    }
}

// =============================================================================
// COST BADGE
// =============================================================================

fn cost_class(cost: f64) -> &'static str {
    if cost < 0.01 {
        "text-green"
    } else if cost < 0.10 {
        "text-yellow"
    } else {
        "text-red"
    }
}

pub fn cost_badge(cost: f64) -> Markup {
    html! {
        span
            title={ "Cost: $" (format!("{:.4}", cost)) }
            class={ "text-xs tabular-nums " (cost_class(cost)) }
        {
            "$" (format!("{:.4}", cost))
        }
    }
}

// =============================================================================
// TOKEN BADGE
// =============================================================================

fn format_tokens(count: u32) -> String {
    if count >= 1000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else {
        count.to_string()
    }
}

pub fn token_badge(
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: Option<u32>,
) -> Markup {
    html! {
        span
            title={
                "Prompt: " (prompt_tokens) ", Completion: " (completion_tokens)
                @if let Some(cached) = cached_tokens {
                    ", Cached: " (cached)
                }
            }
            class="text-xs text-muted-foreground"
        {
            span class="opacity-60" {
                (format_tokens(prompt_tokens))
                " in \u{00B7} "
                (format_tokens(completion_tokens))
                " out"
            }
            @if let Some(cached) = cached_tokens {
                span class="opacity-40" {
                    " (" (format_tokens(cached)) " cached)"
                }
            }
        }
    }
}

// =============================================================================
// LATENCY BADGE
// =============================================================================

fn latency_class(ms: u32) -> &'static str {
    if ms < 1000 {
        "text-green"
    } else if ms < 5000 {
        "text-yellow"
    } else {
        "text-red"
    }
}

pub fn latency_badge(ms: u32) -> Markup {
    let display = if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    };

    html! {
        span
            title={ "Latency: " (ms) "ms" }
            class={ "text-xs tabular-nums " (latency_class(ms)) }
        {
            (display)
        }
    }
}

// =============================================================================
// ATTEMPT BADGE
// =============================================================================

pub fn attempt_badge(attempt: u8, max_attempts: u8) -> Markup {
    html! {
        span
            title={ "Attempt " (attempt) " of " (max_attempts) }
            class="text-xs text-orange tabular-nums"
        {
            (attempt) "/" (max_attempts)
        }
    }
}

// =============================================================================
// TID BADGE (Thread ID)
// =============================================================================

fn tid_class(tid: u8) -> &'static str {
    match tid {
        1 => "text-muted-foreground",
        2 => "text-blue",
        3 => "text-green",
        4 => "text-magenta",
        5 => "text-cyan",
        _ => "text-yellow",
    }
}

pub fn tid_badge(tid: u8) -> Markup {
    html! {
        span
            title={ "Thread ID: " (tid) }
            class={ "text-xs cursor-pointer " (tid_class(tid)) }
        {
            "tid:" (tid)
        }
    }
}

// =============================================================================
// BLOB REF
// =============================================================================

pub fn blob_ref(sha256: &str, bytes: u64, mime: Option<&str>) -> Markup {
    let size_display = if bytes >= 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    };

    let sha_short = &sha256[..8.min(sha256.len())];

    html! {
        span
            title={ "Blob: " (sha256) }
            class="inline-flex items-center gap-2 border border-border bg-card text-cyan text-xs px-2 py-0.5 cursor-pointer"
        {
            "@blob sha256=" (sha_short)
            "\u{00B7}"
            (size_display)
            @if let Some(m) = mime {
                "\u{00B7}"
                (m)
            }
        }
    }
}

// =============================================================================
// REDACTED VALUE
// =============================================================================

pub fn redacted_value(label: &str) -> Markup {
    html! {
        span
            title={ "Redacted: " (label) }
            class="text-xs text-red bg-destructive/10 px-1.5 py-0.5"
        {
            "[redacted:" (label) "]"
        }
    }
}

// =============================================================================
// RESULT ARROW
// =============================================================================

pub fn result_arrow() -> Markup {
    html! {
        span class="text-muted-foreground opacity-60 mx-2" {
            "\u{2192}"
        }
    }
}
