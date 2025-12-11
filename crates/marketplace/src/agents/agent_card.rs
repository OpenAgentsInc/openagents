//! Agent card component - Individual agent listing in the grid
//! Bloomberg-style: dense, tabular, no emojis, sharp edges

use gpui::*;
use theme_oa::{bg, border, text, accent, status, FONT_FAMILY};

use crate::types::{AgentListing, TrustTier};

/// Render an agent card - Bloomberg style (dense, tabular)
pub fn render_agent_card(agent: &AgentListing, is_selected: bool) -> impl IntoElement {
    let (card_bg, card_border) = if is_selected {
        (bg::SELECTED, border::SELECTED)
    } else {
        (bg::CARD, border::DEFAULT)
    };

    // Yellow highlight for name if high score
    let name_color = if agent.terminal_bench_score.unwrap_or(0.0) > 90.0 {
        Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }  // Yellow
    } else {
        text::PRIMARY
    };

    div()
        .flex()
        .flex_col()
        .p(px(8.0))  // Denser padding
        .gap(px(6.0))
        .bg(card_bg)
        .border_1()
        .border_color(card_border)
        // No rounded corners - Bloomberg style
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Header: Name + Tier
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(name_color)
                        .child(agent.name.clone()),
                )
                .child(render_tier_badge_small(agent.trust_tier)),
        )
        // Stats table - Bloomberg dense layout
        .child(render_stats_table(agent))
        // Footer: author + action
        .child(render_footer(agent))
}

/// Render stats as a dense table - Bloomberg style
fn render_stats_table(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .py(px(4.0))
        .border_t_1()
        .border_b_1()
        .border_color(border::SUBTLE)
        // Row 1: Rating + Installs
        .child(
            div()
                .flex()
                .justify_between()
                .child(render_stat_row("RTG", &format!("{:.1}", agent.rating), text::SECONDARY))
                .child(render_stat_row("INST", &format_k(agent.installs), text::SECONDARY)),
        )
        // Row 2: TB Score + Earnings
        .child(
            div()
                .flex()
                .justify_between()
                .child(render_stat_row(
                    "TB",
                    &format!("{}%", agent.terminal_bench_score.unwrap_or(0.0) as u32),
                    if agent.terminal_bench_score.unwrap_or(0.0) > 90.0 {
                        status::SUCCESS
                    } else {
                        text::SECONDARY
                    },
                ))
                .child(render_stat_row(
                    "EARN",
                    &format!("{}", format_k(agent.earnings_total_sats)),
                    status::SUCCESS,
                )),
        )
}

/// Render a single stat row (LABEL: VALUE)
fn render_stat_row(label: &str, value: &str, value_color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .w(px(32.0))
                .child(format!("{}:", label)),
        )
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(value_color)
                .child(value.to_string()),
        )
}

/// Render footer with author and install button
fn render_footer(agent: &AgentListing) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        // Author
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::DIM)
                .child(format!("@{}", agent.author_name)),
        )
        // Install button - Bloomberg style action
        .child(
            div()
                .id(SharedString::from(format!("install-{}", agent.id)))
                .px(px(8.0))
                .py(px(3.0))
                .bg(accent::PRIMARY_MUTED)
                .cursor_pointer()
                .hover(|s| s.bg(accent::PRIMARY))
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::PRIMARY)
                        .child("GET"),
                ),
        )
}

/// Render small tier badge - no rounded corners
fn render_tier_badge_small(tier: TrustTier) -> impl IntoElement {
    div()
        .px(px(4.0))
        .py(px(1.0))
        .bg(tier.bg_color())
        .border_1()
        .border_color(tier.border_color())
        // No rounded corners
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(tier.color())
                .child(tier.label().chars().next().unwrap_or('?').to_string()),  // Just first letter: B/S/G/D
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
