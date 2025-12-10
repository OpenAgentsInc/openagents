//! Trajectory List component for browsing ATIF trajectories
//!
//! Ported from Effuse's hf-trajectory-list.ts
//! Displays a paginated, searchable list of trajectories.

use atif_store::TrajectoryMetadata;
use chrono::{DateTime, Utc};
use gpui::prelude::FluentBuilder;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

// ============================================================================
// Helper Functions
// ============================================================================

/// Format date for display
#[allow(dead_code)]
fn format_date(dt: DateTime<Utc>) -> String {
    dt.format("%b %d, %H:%M").to_string()
}

/// Format session ID for display (show last 8 chars)
#[allow(dead_code)]
fn format_session_id(id: &str) -> String {
    if id.len() > 8 {
        format!("...{}", &id[id.len() - 8..])
    } else {
        id.to_string()
    }
}

// ============================================================================
// Trajectory Item Rendering
// ============================================================================

/// Render a single trajectory item in the list
#[allow(dead_code)]
pub fn render_trajectory_item(
    metadata: &TrajectoryMetadata,
    is_selected: bool,
) -> impl IntoElement {
    let session_id = metadata.session_id.clone();
    let agent_name = metadata.agent_name.clone();
    let model_name = metadata.model_name.clone().unwrap_or_else(|| "unknown".to_string());
    let step_count = metadata.total_steps as i32;
    let created_at = format_date(metadata.created_at);
    let item_status = format!("{:?}", metadata.status);

    let (item_bg, item_border) = if is_selected {
        (bg::SELECTED, border::SELECTED)
    } else {
        (bg::CARD, border::DEFAULT)
    };

    div()
        .p(px(12.0))
        .mb(px(8.0))
        .bg(item_bg)
        .border_1()
        .border_color(item_border)
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // Header row: agent name + date
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(agent_name),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(created_at),
                ),
        )
        // Model row
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .mb(px(4.0))
                .child(format!("model: {}", model_name)),
        )
        // Footer row: session ID, steps, status
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format_session_id(&session_id))
                .child(div().text_color(text::DISABLED).child("•"))
                .child(format!("{} steps", step_count))
                .child(div().text_color(text::DISABLED).child("•"))
                .child(render_status_badge(&item_status)),
        )
}

/// Render status badge
#[allow(dead_code)]
fn render_status_badge(item_status: &str) -> impl IntoElement {
    let (badge_bg, badge_text) = match item_status.to_lowercase().as_str() {
        "completed" => (status::SUCCESS_BG, status::SUCCESS),
        "failed" => (status::ERROR_BG, status::ERROR),
        _ => (status::WARNING_BG, status::WARNING),
    };

    div()
        .px(px(6.0))
        .py(px(2.0))
        .text_size(px(10.0))
        .font_family(FONT_FAMILY)
        .bg(badge_bg)
        .text_color(badge_text)
        .rounded(px(4.0))
        .child(item_status.to_lowercase())
}

// ============================================================================
// Trajectory List Header
// ============================================================================

/// Render the list header with title and count
#[allow(dead_code)]
pub fn render_trajectory_list_header(
    total_count: usize,
    is_collapsed: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(12.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::STRONG)
        .cursor_pointer()
        .hover(|s| s.bg(bg::CARD))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child("Trajectories"),
                )
                .when(total_count > 0, |el| {
                    el.child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("({})", total_count)),
                    )
                }),
        )
        .child(
            div()
                .text_color(text::MUTED)
                .child(if is_collapsed { "▼" } else { "▲" }),
        )
}

// ============================================================================
// Search Input
// ============================================================================

/// Render the search input
#[allow(dead_code)]
pub fn render_search_input(current_query: &str) -> impl IntoElement {
    let query = current_query.to_string();

    div()
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(12.0))
                .py(px(8.0))
                .bg(bg::ELEVATED)
                .border_1()
                .border_color(border::DEFAULT)
                .rounded(px(6.0))
                .child(
                    div()
                        .text_color(text::PLACEHOLDER)
                        .text_size(px(14.0))
                        .child("🔍"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(if query.is_empty() {
                            text::PLACEHOLDER
                        } else {
                            text::PRIMARY
                        })
                        .child(if query.is_empty() {
                            "Search trajectories...".to_string()
                        } else {
                            query
                        }),
                ),
        )
}

// ============================================================================
// Pagination Controls
// ============================================================================

/// Render pagination controls
#[allow(dead_code)]
pub fn render_pagination(
    current_page: usize,
    total_pages: usize,
    has_prev: bool,
    has_next: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(12.0))
        .border_t_1()
        .border_color(border::DEFAULT)
        // Prev button
        .child(
            div()
                .px(px(12.0))
                .py(px(6.0))
                .bg(if has_prev { bg::HOVER } else { bg::SURFACE })
                .text_color(if has_prev { text::PRIMARY } else { text::DISABLED })
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .rounded(px(4.0))
                .when(has_prev, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(bg::CARD))
                })
                .child("← Prev"),
        )
        // Page indicator
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("Page {} of {}", current_page + 1, total_pages.max(1))),
        )
        // Next button
        .child(
            div()
                .px(px(12.0))
                .py(px(6.0))
                .bg(if has_next { bg::HOVER } else { bg::SURFACE })
                .text_color(if has_next { text::PRIMARY } else { text::DISABLED })
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .rounded(px(4.0))
                .when(has_next, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(bg::CARD))
                })
                .child("Next →"),
        )
}

// ============================================================================
// Full List Rendering
// ============================================================================

/// Render the complete trajectory list
#[allow(dead_code)]
pub fn render_trajectory_list(
    trajectories: &[TrajectoryMetadata],
    selected_session_id: Option<&str>,
    total_count: usize,
    current_page: usize,
    page_size: usize,
    is_loading: bool,
    error: Option<&str>,
    is_collapsed: bool,
    search_query: &str,
) -> impl IntoElement {
    let total_pages = (total_count + page_size - 1) / page_size;
    let has_prev = current_page > 0;
    let has_next = current_page + 1 < total_pages;

    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(border::STRONG)
        .bg(bg::SURFACE)
        .overflow_hidden()
        // Header
        .child(render_trajectory_list_header(total_count, is_collapsed))
        // Content (only if not collapsed)
        .when(!is_collapsed, |el| {
            el
                // Search input
                .child(render_search_input(search_query))
                // Content area
                .child(
                    div()
                        .max_h(px(400.0))
                        .overflow_hidden()
                        .px(px(16.0))
                        .py(px(12.0))
                        .child(render_list_content(
                            trajectories,
                            selected_session_id,
                            is_loading,
                            error,
                            search_query,
                        )),
                )
                // Pagination
                .child(render_pagination(current_page, total_pages, has_prev, has_next))
        })
}

/// Render the list content (loading, error, empty, or items)
#[allow(dead_code)]
fn render_list_content(
    trajectories: &[TrajectoryMetadata],
    selected_session_id: Option<&str>,
    is_loading: bool,
    error: Option<&str>,
    search_query: &str,
) -> impl IntoElement {
    if is_loading {
        return div()
            .py(px(32.0))
            .text_size(px(14.0))
            .font_family(FONT_FAMILY)
            .text_color(text::MUTED)
            .flex()
            .items_center()
            .justify_center()
            .child("Loading trajectories...")
            .into_any_element();
    }

    if let Some(err) = error {
        return div()
            .py(px(32.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status::ERROR)
                    .child("Error loading trajectories"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(err.to_string()),
            )
            .into_any_element();
    }

    if trajectories.is_empty() {
        let message = if search_query.is_empty() {
            "No trajectories found".to_string()
        } else {
            format!("No trajectories match \"{}\"", search_query)
        };

        return div()
            .py(px(32.0))
            .text_size(px(14.0))
            .font_family(FONT_FAMILY)
            .text_color(text::MUTED)
            .flex()
            .items_center()
            .justify_center()
            .child(message)
            .into_any_element();
    }

    // Render trajectory items
    div()
        .flex()
        .flex_col()
        .children(trajectories.iter().map(|traj| {
            let is_selected = selected_session_id == Some(&traj.session_id);
            render_trajectory_item(traj, is_selected)
        }))
        .into_any_element()
}
