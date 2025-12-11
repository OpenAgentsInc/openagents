//! Analytics view component - Traffic and usage metrics

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{AnalyticsSummary, AnalyticsRange, AnalyticsDataPoint};

/// Render the analytics view
pub fn render_analytics_view(analytics: &AnalyticsSummary, range: AnalyticsRange) -> impl IntoElement {
    div()
        .id("analytics-view")
        .flex()
        .flex_col()
        .flex_1()
        .bg(bg::APP)
        // Header with range selector
        .child(
            div()
                .h(px(44.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("ANALYTICS"),
                )
                // Range selector
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .children(AnalyticsRange::all().iter().map(|&r| {
                            render_range_button(r, r == range)
                        })),
                ),
        )
        // Summary cards
        .child(
            div()
                .w_full()
                .flex()
                .gap(px(12.0))
                .p(px(16.0))
                .child(render_stat_card("PAGE VIEWS", &format_number(analytics.page_views), Some("+12%"), status::SUCCESS))
                .child(render_stat_card("UNIQUE VISITORS", &format_number(analytics.unique_visitors), Some("+8%"), status::SUCCESS))
                .child(render_stat_card("AVG SESSION", &format_duration(analytics.avg_session_duration_secs), Some("-2%"), status::ERROR))
                .child(render_stat_card("BOUNCE RATE", &format!("{:.1}%", analytics.bounce_rate), Some("-5%"), status::SUCCESS)),
        )
        // Charts section
        .child(
            div()
                .id("analytics-scroll")
                .flex_1()
                .overflow_y_scroll()
                .p(px(16.0))
                .gap(px(16.0))
                .flex()
                .flex_col()
                // Traffic chart
                .child(render_traffic_chart(&analytics.traffic_by_day))
                // Bottom row: top pages + breakdowns
                .child(
                    div()
                        .flex()
                        .gap(px(16.0))
                        // Top pages
                        .child(render_top_pages(&analytics.top_pages))
                        // Traffic breakdowns
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(16.0))
                                .child(render_breakdown("BY COUNTRY", &analytics.traffic_by_country))
                                .child(render_breakdown("BY DEVICE", &analytics.traffic_by_device)),
                        ),
                ),
        )
}

/// Render a range button
fn render_range_button(range: AnalyticsRange, is_active: bool) -> impl IntoElement {
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .px(px(10.0))
        .py(px(4.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(range.label()),
        )
}

/// Render a stat card
fn render_stat_card(label: &str, value: &str, change: Option<&str>, change_color: Hsla) -> impl IntoElement {
    let label = label.to_string();
    let value = value.to_string();
    let change = change.map(|s| s.to_string());
    div()
        .flex_1()
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(8.0))
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(24.0))
                        .font_family(FONT_FAMILY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child(value),
                )
                .when(change.is_some(), |el| {
                    el.child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(change_color)
                            .pb(px(4.0))
                            .child(change.clone().unwrap()),
                    )
                }),
        )
}

/// Render the traffic chart
fn render_traffic_chart(data: &[AnalyticsDataPoint]) -> impl IntoElement {
    let max_value = data.iter().map(|d| d.value).max().unwrap_or(1) as f32;

    div()
        .w_full()
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(16.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("TRAFFIC"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(render_chart_legend("Page Views", Hsla { h: 0.55, s: 0.7, l: 0.5, a: 1.0 }))
                        .child(render_chart_legend("Visitors", Hsla { h: 0.14, s: 0.8, l: 0.5, a: 1.0 })),
                ),
        )
        // Chart
        .child(
            div()
                .h(px(120.0))
                .w_full()
                .flex()
                .items_end()
                .gap(px(8.0))
                .children(data.iter().map(|point| {
                    let height = (point.value as f32 / max_value * 100.0).max(4.0);
                    render_chart_bar(&point.label, height)
                })),
        )
}

/// Render a chart legend item
fn render_chart_legend(label: &str, color: Hsla) -> impl IntoElement {
    let label = label.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .w(px(8.0))
                .h(px(8.0))
                .bg(color),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
}

/// Render a chart bar
fn render_chart_bar(label: &str, height_percent: f32) -> impl IntoElement {
    let label = label.to_string();
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        // Bar
        .child(
            div()
                .w_full()
                .h(px(height_percent))
                .bg(Hsla { h: 0.55, s: 0.7, l: 0.5, a: 1.0 }),
        )
        // Label
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
}

/// Render top pages list
fn render_top_pages(pages: &[(String, u64)]) -> impl IntoElement {
    let max_views = pages.first().map(|(_, v)| *v).unwrap_or(1) as f32;

    div()
        .flex_1()
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(12.0))
                .child("TOP PAGES"),
        )
        .children(pages.iter().map(|(page, views)| {
            let bar_width = (*views as f32 / max_views * 100.0).max(10.0);
            div()
                .w_full()
                .mb(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(2.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(page.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format_number(*views)),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(4.0))
                        .bg(bg::APP)
                        .child(
                            div()
                                .h_full()
                                .w(px(bar_width * 2.0))
                                .bg(Hsla { h: 0.55, s: 0.6, l: 0.5, a: 1.0 }),
                        ),
                )
        }))
}

/// Render a breakdown section
fn render_breakdown(title: &str, data: &[(String, u64)]) -> impl IntoElement {
    let title = title.to_string();
    let total: u64 = data.iter().map(|(_, v)| v).sum();

    div()
        .w(px(200.0))
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(12.0))
                .child(title),
        )
        .children(data.iter().map(|(name, value)| {
            let percent = if total > 0 { (*value as f32 / total as f32 * 100.0) } else { 0.0 };
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(6.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("{:.0}%", percent)),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(format_number(*value)),
                        ),
                )
        }))
}

/// Format a number with K/M suffixes
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format duration in seconds to human-readable
fn format_duration(secs: u32) -> String {
    if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
