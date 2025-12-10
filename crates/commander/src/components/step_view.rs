//! Step View component for ATIF trajectory steps
//!
//! Ported from Effuse's atif-details.ts
//! Displays step-by-step details including tool calls, observations, and metrics.

use atif::{Metrics, Observation, Step, StepSource, ToolCall};
use chrono::{DateTime, Utc};
use gpui::prelude::FluentBuilder;
use gpui::*;
use theme::{accent, bg, border, source, status, text, FONT_FAMILY};

// ============================================================================
// Source Badge Colors
// ============================================================================

/// Get colors for source badge (background, text, border)
pub fn source_badge_colors(source: &StepSource) -> (Hsla, Hsla, Hsla) {
    match source {
        StepSource::User => (source::USER_BG, source::USER_TEXT, source::USER_BORDER),
        StepSource::Agent => (source::AGENT_BG, source::AGENT_TEXT, source::AGENT_BORDER),
        StepSource::System => (source::SYSTEM_BG, source::SYSTEM_TEXT, source::SYSTEM_BORDER),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format timestamp as HH:MM:SS
fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.format("%H:%M:%S").to_string()
}

/// Format optional timestamp
fn format_optional_timestamp(dt: Option<DateTime<Utc>>) -> String {
    dt.map(format_timestamp).unwrap_or_else(|| "--:--:--".to_string())
}

/// Format JSON value for display (truncated if too long)
fn format_json(value: &serde_json::Value, max_len: usize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(s) if s.len() > max_len => format!("{}...", &s[..max_len]),
        Ok(s) => s,
        Err(_) => value.to_string(),
    }
}

/// Truncate a string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

// ============================================================================
// Source Badge Rendering
// ============================================================================

/// Render a source badge
pub fn render_source_badge(source: &StepSource) -> impl IntoElement {
    let (bg, text, border) = source_badge_colors(source);
    let label = match source {
        StepSource::User => "USER",
        StepSource::Agent => "AGENT",
        StepSource::System => "SYSTEM",
    };

    div()
        .px(px(6.0))
        .py(px(2.0))
        .text_size(px(10.0))
        .font_family(FONT_FAMILY)
        .bg(bg)
        .text_color(text)
        .border_1()
        .border_color(border)
        
        .flex_shrink_0()
        .child(label)
}

// ============================================================================
// Tool Call Rendering
// ============================================================================

/// Render a single tool call
pub fn render_tool_call(tool_call: &ToolCall) -> impl IntoElement {
    div()
        .p(px(8.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        
        .mb(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .mb(px(4.0))
                .child(
                    div()
                        .text_color(accent::SECONDARY) // violet
                        .child("🔧"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::SECONDARY)
                        .child(tool_call.function_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("id: {}", truncate(&tool_call.tool_call_id, 12))),
                ),
        )
        .child(
            div()
                .p(px(8.0))
                .bg(bg::SURFACE)
                
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .overflow_hidden()
                .child(format_json(&tool_call.arguments, 500)),
        )
}

/// Render tool calls section
pub fn render_tool_calls(tool_calls: &[ToolCall]) -> impl IntoElement {
    div()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .mb(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("TOOL CALLS"),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .bg(accent::SECONDARY_MUTED)
                        .text_color(accent::SECONDARY)
                        
                        .child(format!("{}", tool_calls.len())),
                ),
        )
        .children(tool_calls.iter().map(render_tool_call))
}

// ============================================================================
// Observation Rendering
// ============================================================================

/// Render observation results
pub fn render_observation(observation: &Observation) -> impl IntoElement {
    div()
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(8.0))
                .child("OBSERVATION"),
        )
        .children(observation.results.iter().map(|result| {
            div()
                .p(px(8.0))
                .bg(bg::ELEVATED)
                .border_1()
                .border_color(border::DEFAULT)
                
                .mb(px(8.0))
                .when_some(result.source_call_id.as_ref(), |el, call_id| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .mb(px(4.0))
                            .child(format!("call: {}", truncate(call_id, 12))),
                    )
                })
                .when_some(result.content.as_ref(), |el, content| {
                    el.child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(accent::TERTIARY) // emerald
                            .overflow_hidden()
                            .child(truncate(content, 1000)),
                    )
                })
                .when_some(result.subagent_trajectory_ref.as_ref(), |el, refs| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(accent::PRIMARY)
                                    .child("Subagent:"),
                            )
                            .children(refs.iter().map(|r| {
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(accent::PRIMARY)
                                    .child(truncate(&r.session_id, 16))
                            })),
                    )
                })
        }))
}

// ============================================================================
// Metrics Rendering
// ============================================================================

/// Render step metrics
pub fn render_metrics(metrics: &Metrics) -> impl IntoElement {
    div()
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(8.0))
                .child("METRICS"),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(16.0))
                .text_size(px(12.0))
                .when_some(metrics.prompt_tokens, |el, tokens| {
                    el.child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Prompt:"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::SECONDARY)
                                    .child(format!("{}", tokens)),
                            ),
                    )
                })
                .when_some(metrics.completion_tokens, |el, tokens| {
                    el.child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Completion:"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::SECONDARY)
                                    .child(format!("{}", tokens)),
                            ),
                    )
                })
                .when_some(metrics.cached_tokens, |el, tokens| {
                    el.child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Cached:"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(status::WARNING)
                                    .child(format!("{}", tokens)),
                            ),
                    )
                })
                .when_some(metrics.cost_usd, |el, cost| {
                    el.child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Cost:"),
                            )
                            .child(
                                div()
                                    .font_family(FONT_FAMILY)
                                    .text_color(status::SUCCESS)
                                    .child(format!("${:.4}", cost)),
                            ),
                    )
                }),
        )
}

// ============================================================================
// Step Header Rendering
// ============================================================================

/// Render step header (collapsed view)
pub fn render_step_header(step: &Step, is_expanded: bool) -> impl IntoElement {
    let tool_count = step.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0);

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(10.0))
        .bg(bg::CARD)
        .hover(|s| s.bg(bg::HOVER))
        .cursor_pointer()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .flex_1()
                .min_w_0()
                // Step ID
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .flex_shrink_0()
                        .child(format!("#{}", step.step_id)),
                )
                // Source badge
                .child(render_source_badge(&step.source))
                // Timestamp
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .flex_shrink_0()
                        .child(format_optional_timestamp(step.timestamp)),
                )
                // Model (if agent)
                .when_some(step.model_name.as_ref(), |el, model| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(truncate(model, 20)),
                    )
                })
                // Tool count (if any)
                .when(tool_count > 0, |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_color(accent::SECONDARY)
                                    .text_size(px(12.0))
                                    .child("🔧"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(accent::SECONDARY)
                                    .child(format!(
                                        "{} tool{}",
                                        tool_count,
                                        if tool_count == 1 { "" } else { "s" }
                                    )),
                            ),
                    )
                })
                // Message preview (truncated)
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .truncate()
                        .flex_1()
                        .min_w_0()
                        .child(truncate(&step.message, 60)),
                ),
        )
        // Expand/collapse indicator
        .child(
            div()
                .text_color(text::MUTED)
                .flex_shrink_0()
                .ml(px(8.0))
                .child(if is_expanded { "▲" } else { "▼" }),
        )
}

// ============================================================================
// Step Details Rendering (Expanded)
// ============================================================================

/// Render step details (expanded view)
pub fn render_step_details(step: &Step) -> impl IntoElement {
    div()
        .px(px(16.0))
        .py(px(12.0))
        .bg(bg::ELEVATED)
        .flex()
        .flex_col()
        .gap(px(16.0))
        // Message content
        .child(
            div()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(8.0))
                        .child("MESSAGE"),
                )
                .child(
                    div()
                        .p(px(12.0))
                        .bg(bg::SURFACE)
                        .border_1()
                        .border_color(border::DEFAULT)
                        
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .line_height(px(20.0))
                        .overflow_hidden()
                        .child(step.message.clone()),
                ),
        )
        // Reasoning content (if any)
        .when_some(step.reasoning_content.as_ref(), |el, reasoning| {
            el.child(
                div()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::INFO)
                            .mb(px(8.0))
                            .child("REASONING"),
                    )
                    .child(
                        div()
                            .p(px(12.0))
                            .bg(status::INFO_BG)
                            .border_1()
                            .border_color(status::INFO_BORDER)
                            
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(accent::PRIMARY)
                            .line_height(px(20.0))
                            .child(reasoning.clone()),
                    ),
            )
        })
        // Tool calls (if any)
        .when_some(step.tool_calls.as_ref(), |el, tool_calls| {
            if tool_calls.is_empty() {
                el
            } else {
                el.child(render_tool_calls(tool_calls))
            }
        })
        // Observation (if any)
        .when_some(step.observation.as_ref(), |el, observation| {
            if observation.results.is_empty() {
                el
            } else {
                el.child(render_observation(observation))
            }
        })
        // Metrics (if any)
        .when_some(step.metrics.as_ref(), |el, metrics| {
            el.child(render_metrics(metrics))
        })
}

// ============================================================================
// Full Step Rendering
// ============================================================================

/// Render a complete step (header + optional details)
pub fn render_step(step: &Step, is_expanded: bool) -> impl IntoElement {
    div()
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(render_step_header(step, is_expanded))
        .when(is_expanded, |el| el.child(render_step_details(step)))
}

// ============================================================================
// Steps List Rendering
// ============================================================================

/// Render a list of steps with expansion state
pub fn render_steps_list(steps: &[Step], expanded_step_ids: &std::collections::HashSet<i64>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .children(steps.iter().map(|step| {
            let is_expanded = expanded_step_ids.contains(&step.step_id);
            render_step(step, is_expanded)
        }))
}
