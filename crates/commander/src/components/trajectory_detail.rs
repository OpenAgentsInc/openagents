//! Trajectory Detail component for displaying full ATIF trajectories
//!
//! Ported from Effuse's hf-trajectory-detail.ts
//! Displays a full trajectory with metadata, step-by-step accordion view,
//! tool calls, observations, and metrics.

use crate::components::step_view::{render_source_badge, render_step_details, render_step_header};
use atif::{Step, Trajectory};
use chrono::{DateTime, Utc};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashSet;

// ============================================================================
// Helper Functions
// ============================================================================

/// Format timestamp for display (HH:MM:SS)
fn format_timestamp(dt: Option<DateTime<Utc>>) -> String {
    match dt {
        Some(dt) => dt.format("%H:%M:%S").to_string(),
        None => "--:--:--".to_string(),
    }
}

/// Format date for display (Mon DD, YYYY HH:MM)
fn format_date(dt: Option<DateTime<Utc>>) -> String {
    match dt {
        Some(dt) => dt.format("%b %d, %Y %H:%M").to_string(),
        None => "unknown".to_string(),
    }
}

// ============================================================================
// Trajectory Detail Header
// ============================================================================

/// Render the detail header with title and collapse toggle
pub fn render_detail_header(is_collapsed: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(12.0))
        .bg(hsla(0.0, 0.0, 0.06, 0.4))
        .border_b_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.6))
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0.0, 0.0, 0.08, 0.6)))
        .child(
            div()
                .text_size(px(14.0))
                .font_family("Berkeley Mono")
                .text_color(hsla(0.0, 0.0, 0.9, 1.0))
                .child("Trajectory Details"),
        )
        .child(
            div()
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child(if is_collapsed { "▼" } else { "▲" }),
        )
}

// ============================================================================
// Metadata Section
// ============================================================================

/// Render the metadata row
fn render_metadata_row(label: &str, value: String) -> impl IntoElement {
    let label_owned = label.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(12.0))
        .child(
            div()
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .font_family("Berkeley Mono")
                .child(format!("{}:", label_owned)),
        )
        .child(
            div()
                .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                .font_family("Berkeley Mono")
                .child(value),
        )
}

/// Render the trajectory metadata section
pub fn render_trajectory_metadata(trajectory: &Trajectory) -> impl IntoElement {
    let session_id = trajectory.session_id.clone();
    let agent_name = trajectory.agent.name.clone();
    let agent_version = trajectory.agent.version.clone();
    let model_name = trajectory
        .agent
        .model_name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let step_count = trajectory.steps.len();

    // Get the first step's timestamp for the date
    let first_timestamp = trajectory.steps.first().and_then(|s| s.timestamp);
    let date_str = format_date(first_timestamp);

    // Try to get task/episode from extra
    let task = trajectory
        .extra
        .as_ref()
        .and_then(|e| e.get("task"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let episode = trajectory
        .extra
        .as_ref()
        .and_then(|e| e.get("episode"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    div()
        .px(px(16.0))
        .py(px(12.0))
        .bg(hsla(0.0, 0.0, 0.04, 0.2))
        .border_b_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.4))
        .flex()
        .flex_col()
        .gap(px(6.0))
        // Session ID
        .child(render_metadata_row("Session", session_id))
        // Agent info
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child("Agent:"),
                )
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                        .font_family("Berkeley Mono")
                        .child(agent_name),
                )
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child(format!("v{}", agent_version)),
                )
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.4, 1.0))
                        .font_family("Berkeley Mono")
                        .child(format!("({})", model_name)),
                ),
        )
        // Task/Episode
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child("Task:"),
                )
                .child(div().text_color(hsla(0.0, 0.0, 0.8, 1.0)).font_family("Berkeley Mono").child(task))
                .child(div().text_color(hsla(0.0, 0.0, 0.3, 1.0)).child("•"))
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child("Episode:"),
                )
                .child(div().text_color(hsla(0.0, 0.0, 0.8, 1.0)).font_family("Berkeley Mono").child(episode)),
        )
        // Steps/Date
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child("Steps:"),
                )
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                        .font_family("Berkeley Mono")
                        .child(format!("{}", step_count)),
                )
                .child(div().text_color(hsla(0.0, 0.0, 0.3, 1.0)).child("•"))
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .font_family("Berkeley Mono")
                        .child("Date:"),
                )
                .child(
                    div()
                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                        .font_family("Berkeley Mono")
                        .child(date_str),
                ),
        )
}

// ============================================================================
// Step Row
// ============================================================================

/// Render a single step row with header
pub fn render_step_row(step: &Step, is_expanded: bool) -> impl IntoElement {
    let step_id = step.step_id;
    let timestamp = format_timestamp(step.timestamp);
    let source = &step.source;

    // Count tool calls
    let tool_call_count = step.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0);
    let has_observation = step.observation.is_some();

    // Step header row
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(8.0))
        .bg(hsla(0.0, 0.0, 0.04, 0.4))
        .border_b_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.4))
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0.0, 0.0, 0.06, 0.6)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                // Step number
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family("Berkeley Mono")
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .child(format!("#{}", step_id)),
                )
                // Source badge
                .child(render_source_badge(source))
                // Timestamp
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family("Berkeley Mono")
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .child(timestamp),
                )
                // Tool calls indicator
                .when(tool_call_count > 0, |el| {
                    el.child(
                        div()
                            .text_size(px(11.0))
                            .font_family("Berkeley Mono")
                            .text_color(hsla(0.75, 0.5, 0.6, 1.0)) // violet
                            .child(format!(
                                "🔧 {} tool{}",
                                tool_call_count,
                                if tool_call_count > 1 { "s" } else { "" }
                            )),
                    )
                })
                // Observation indicator
                .when(has_observation, |el| {
                    el.child(
                        div()
                            .text_size(px(11.0))
                            .font_family("Berkeley Mono")
                            .text_color(hsla(0.38, 0.5, 0.6, 1.0)) // green
                            .child("✓ obs"),
                    )
                }),
        )
        .child(
            div()
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child(if is_expanded { "▲" } else { "▼" }),
        );

    // Build the full row
    div()
        .border_b_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.4))
        .child(header)
        .when(is_expanded, |el| el.child(render_step_details(step)))
}

// ============================================================================
// Steps List
// ============================================================================

/// Render the list of steps with expansion state
pub fn render_detail_steps(steps: &[Step], expanded_step_ids: &HashSet<i64>) -> impl IntoElement {
    div()
        .max_h(px(500.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .children(
            steps
                .iter()
                .map(|step| {
                    let is_expanded = expanded_step_ids.contains(&step.step_id);
                    render_step_row(step, is_expanded)
                })
                .collect::<Vec<_>>(),
        )
}

// ============================================================================
// Final Metrics
// ============================================================================

/// Render the final metrics section
pub fn render_final_metrics(trajectory: &Trajectory) -> impl IntoElement {
    let metrics = &trajectory.final_metrics;

    match metrics {
        Some(m) => {
            let prompt_tokens = m.total_prompt_tokens.unwrap_or(0);
            let completion_tokens = m.total_completion_tokens.unwrap_or(0);
            let cached_tokens = m.total_cached_tokens.unwrap_or(0);
            let cost = m.total_cost_usd.unwrap_or(0.0);
            let total_steps = m.total_steps.unwrap_or(0);

            div()
                .px(px(16.0))
                .py(px(12.0))
                .bg(hsla(0.0, 0.0, 0.04, 0.2))
                .border_t_1()
                .border_color(hsla(0.0, 0.0, 0.15, 0.4))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family("Berkeley Mono")
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .mb(px(8.0))
                        .child("Final Metrics"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .text_size(px(11.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child("Total Steps:"),
                                )
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child(format!("{}", total_steps)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child("Prompt:"),
                                )
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child(format!("{}", prompt_tokens)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child("Completion:"),
                                )
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child(format!("{}", completion_tokens)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child("Cached:"),
                                )
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child(format!("{}", cached_tokens)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child("Cost:"),
                                )
                                .child(
                                    div()
                                        .text_color(hsla(0.38, 0.5, 0.6, 1.0))
                                        .font_family("Berkeley Mono")
                                        .child(format!("${:.4}", cost)),
                                ),
                        ),
                )
        }
        None => div(),
    }
}

// ============================================================================
// Empty/Loading/Error States
// ============================================================================

/// Render the loading state
pub fn render_loading_state() -> impl IntoElement {
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.6))
        .bg(hsla(0.0, 0.0, 0.03, 0.8))
        .overflow_hidden()
        .child(render_detail_header(false))
        .child(
            div()
                .px(px(16.0))
                .py(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .font_family("Berkeley Mono")
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child("Loading trajectory..."),
        )
}

/// Render the error state
pub fn render_error_state(error: &str) -> impl IntoElement {
    let error_owned = error.to_string();
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.6))
        .bg(hsla(0.0, 0.0, 0.03, 0.8))
        .overflow_hidden()
        .child(render_detail_header(false))
        .child(
            div()
                .px(px(16.0))
                .py(px(32.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family("Berkeley Mono")
                        .text_color(hsla(0.0, 0.6, 0.6, 1.0))
                        .child("Error loading trajectory"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family("Berkeley Mono")
                        .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                        .child(error_owned),
                ),
        )
}

/// Render the empty state (no trajectory selected)
pub fn render_empty_state() -> impl IntoElement {
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.6))
        .bg(hsla(0.0, 0.0, 0.03, 0.8))
        .overflow_hidden()
        .child(render_detail_header(false))
        .child(
            div()
                .px(px(16.0))
                .py(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .font_family("Berkeley Mono")
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child("No trajectory selected. Click a trajectory in the sidebar to view details."),
        )
}

// ============================================================================
// Full Trajectory Detail View
// ============================================================================

/// Render the complete trajectory detail view
pub fn render_trajectory_detail(
    trajectory: Option<&Trajectory>,
    expanded_step_ids: &HashSet<i64>,
    is_loading: bool,
    error: Option<&str>,
    is_collapsed: bool,
) -> impl IntoElement {
    // Loading state
    if is_loading {
        return render_loading_state().into_any_element();
    }

    // Error state
    if let Some(err) = error {
        return render_error_state(err).into_any_element();
    }

    // Empty state
    let Some(traj) = trajectory else {
        return render_empty_state().into_any_element();
    };

    // Collapsed state
    if is_collapsed {
        return div()
            .rounded(px(12.0))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.15, 0.6))
            .bg(hsla(0.0, 0.0, 0.03, 0.8))
            .overflow_hidden()
            .child(render_detail_header(true))
            .into_any_element();
    }

    // Full trajectory view
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(hsla(0.0, 0.0, 0.15, 0.6))
        .bg(hsla(0.0, 0.0, 0.03, 0.8))
        .overflow_hidden()
        // Header
        .child(render_detail_header(false))
        // Metadata
        .child(render_trajectory_metadata(traj))
        // Steps list
        .child(render_detail_steps(&traj.steps, expanded_step_ids))
        // Final metrics
        .child(render_final_metrics(traj))
        .into_any_element()
}
