//! Agent card component - Individual agent listing in the grid

use gpui::*;
use theme::{bg, border, text, accent, status, FONT_FAMILY};

use crate::types::{AgentListing, TrustTier};

/// Render an agent card
pub fn render_agent_card(agent: &AgentListing, is_selected: bool) -> impl IntoElement {
    let (card_bg, card_border) = if is_selected {
        (bg::SELECTED, border::SELECTED)
    } else {
        (bg::CARD, border::DEFAULT)
    };

    div()
        .flex()
        .flex_col()
        .p(px(16.0))
        .gap(px(12.0))
        .bg(card_bg)
        .border_1()
        .border_color(card_border)
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Thumbnail placeholder
        .child(render_thumbnail())
        // Title and description
        .child(render_info(agent))
        // Stats row
        .child(render_stats(agent))
        // Footer with author, tier, install button
        .child(render_footer(agent))
}

/// Render the thumbnail placeholder
fn render_thumbnail() -> impl IntoElement {
    div()
        .h(px(100.0))
        .w_full()
        .bg(bg::ELEVATED)
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(32.0))
                .text_color(text::DIM)
                .child("🤖"),
        )
}

/// Render agent name and description
fn render_info(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(16.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(agent.name.clone()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .line_height(px(18.0))
                .max_h(px(36.0))
                .overflow_hidden()
                .child(agent.description.clone()),
        )
}

/// Render stats row (rating, installs, benchmark, earnings)
fn render_stats(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(12.0))
        // Rating
        .child(render_stat(
            "⭐",
            &format!("{:.1} ({:.1}k)", agent.rating, agent.review_count as f32 / 1000.0),
            status::WARNING,
        ))
        // Installs
        .child(render_stat(
            "📦",
            &format_k(agent.installs),
            text::SECONDARY,
        ))
        // Benchmark score
        .child(render_stat(
            "🎯",
            &format!("TB: {}%", agent.terminal_bench_score.unwrap_or(0.0) as u32),
            if agent.terminal_bench_score.unwrap_or(0.0) > 90.0 {
                status::SUCCESS
            } else {
                text::SECONDARY
            },
        ))
        // Earnings
        .child(render_stat(
            "💰",
            &format!("{} sats", format_k(agent.earnings_total_sats)),
            status::SUCCESS,
        ))
}

/// Render a single stat
fn render_stat(icon: &str, value: &str, color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .child(icon.to_string()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(value.to_string()),
        )
}

/// Render footer with author, tier badge, and install button
fn render_footer(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .pt(px(8.0))
        .border_t_1()
        .border_color(border::SUBTLE)
        // Author
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("by {}", agent.author_name)),
        )
        // Tier badge and install button
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(render_tier_badge_small(agent.trust_tier))
                .child(render_install_button()),
        )
}

/// Render small tier badge
fn render_tier_badge_small(tier: TrustTier) -> impl IntoElement {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .bg(tier.bg_color())
        .border_1()
        .border_color(tier.border_color())
        .rounded(px(3.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(tier.color())
                .child(tier.label().to_string()),
        )
}

/// Render install button
fn render_install_button() -> impl IntoElement {
    div()
        .px(px(12.0))
        .py(px(6.0))
        .bg(accent::PRIMARY_MUTED)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(accent::PRIMARY))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(accent::PRIMARY)
                .child("INSTALL"),
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
