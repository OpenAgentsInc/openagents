//! Agent detail panel - Slide-out panel with full agent info

use gpui::*;
use theme_oa::{bg, border, text, accent, status, FONT_FAMILY};

use crate::types::AgentListing;

/// Width of the detail panel
pub const DETAIL_PANEL_WIDTH: f32 = 320.0;

/// Render the agent detail panel
pub fn render_agent_detail(agent: &AgentListing) -> impl IntoElement {
    div()
        .w(px(DETAIL_PANEL_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        // Header
        .child(render_header(agent))
        // Content (scrollable)
        .child(
            div()
                .id("agent-detail-content")
                .flex_1()
                .overflow_y_scroll()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(20.0))
                // Benchmarks section
                .child(render_benchmarks_section(agent))
                // Earnings section
                .child(render_earnings_section(agent))
                // Description section
                .child(render_description_section(agent))
                // Reviews section
                .child(render_reviews_section(agent)),
        )
        // Footer with actions
        .child(render_footer())
}

/// Render the detail header with thumbnail and title
fn render_header(agent: &AgentListing) -> impl IntoElement {
    div()
        .p(px(16.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .flex()
        .flex_col()
        .gap(px(12.0))
        // Large thumbnail
        .child(
            div()
                .h(px(140.0))
                .w_full()
                .bg(bg::ELEVATED)
                .rounded(px(8.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(48.0))
                        .text_color(text::DIM)
                        .child("🤖"),
                ),
        )
        // Title and author
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(agent.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("by {} • v{}", agent.author_name, agent.version)),
                ),
        )
        // Rating
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .child("⭐"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::WARNING)
                        .child(format!("{:.1}", agent.rating)),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("({} reviews)", format_k(agent.review_count as u64))),
                ),
        )
}

/// Render benchmarks section
fn render_benchmarks_section(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_section_header("BENCHMARKS"))
        // Terminal-Bench score
        .child(render_progress_bar(
            "Terminal-Bench",
            agent.terminal_bench_score.unwrap_or(0.0),
        ))
        // GYM score
        .child(render_progress_bar(
            "GYM Score",
            agent.gym_score.unwrap_or(0.0),
        ))
}

/// Render earnings section
fn render_earnings_section(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_section_header("EARNINGS"))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Total Earned"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::SUCCESS)
                        .child(format!("{} sats", format_k(agent.earnings_total_sats))),
                ),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Revenue Share"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(format!("{}%", agent.revenue_share_percent)),
                ),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Installs"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(format_k(agent.installs)),
                ),
        )
}

/// Render description section
fn render_description_section(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("DESCRIPTION"))
        .child(
            div()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .line_height(px(20.0))
                .child(agent.description.clone()),
        )
}

/// Render reviews section (placeholder)
fn render_reviews_section(_agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("REVIEWS"))
        .child(
            div()
                .p(px(12.0))
                .bg(bg::ELEVATED)
                .rounded(px(6.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Reviews coming soon..."),
                ),
        )
}

/// Render section header
fn render_section_header(title: &str) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_family(FONT_FAMILY)
        .text_color(text::MUTED)
        .child(title.to_string())
}

/// Render a progress bar with label and percentage
fn render_progress_bar(label: &str, percentage: f32) -> impl IntoElement {
    let bar_color = if percentage >= 90.0 {
        status::SUCCESS
    } else if percentage >= 70.0 {
        status::WARNING
    } else {
        status::ERROR
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(bar_color)
                        .child(format!("{}%", percentage as u32)),
                ),
        )
        .child(
            div()
                .h(px(6.0))
                .w_full()
                .bg(bg::ELEVATED)
                .rounded(px(3.0))
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(percentage / 100.0))
                        .bg(bar_color)
                        .rounded(px(3.0)),
                ),
        )
}

/// Render footer with action buttons
fn render_footer() -> impl IntoElement {
    div()
        .p(px(16.0))
        .border_t_1()
        .border_color(border::DEFAULT)
        .flex()
        .flex_col()
        .gap(px(8.0))
        // Install button
        .child(
            div()
                .w_full()
                .py(px(12.0))
                .bg(accent::PRIMARY)
                .rounded(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(accent::SECONDARY))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::BRIGHT)
                        .child("INSTALL AGENT"),
                ),
        )
        // Publish similar button
        .child(
            div()
                .w_full()
                .py(px(10.0))
                .bg(bg::ELEVATED)
                .border_1()
                .border_color(border::DEFAULT)
                .rounded(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(bg::HOVER))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .child("PUBLISH SIMILAR"),
                ),
        )
}

/// Format number with K suffix
fn format_k(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
