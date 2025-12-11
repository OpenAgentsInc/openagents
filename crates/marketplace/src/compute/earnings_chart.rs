//! Earnings chart component - Bar chart visualization of earnings

use gpui::*;
use theme_oa::{bg, border, text, accent, FONT_FAMILY};

use crate::types::{TimeRange, EarningsDataPoint};

/// Render the earnings chart panel
pub fn render_earnings_chart(
    data: &[EarningsDataPoint],
    selected_range: TimeRange,
    total_sats: u64,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p(px(16.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        // Header with title and time range selector
        .child(render_chart_header(selected_range))
        // Chart area
        .child(render_chart_bars(data))
        // Footer with totals
        .child(render_chart_footer(total_sats))
}

/// Render chart header with title and time range buttons
fn render_chart_header(selected_range: TimeRange) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("YOUR EARNINGS"),
        )
        .child(
            div()
                .flex()
                .gap(px(2.0))
                .children(TimeRange::all().iter().map(|&range| {
                    render_time_range_button(range, range == selected_range)
                })),
        )
}

/// Render a time range button
fn render_time_range_button(range: TimeRange, is_selected: bool) -> impl IntoElement {
    let (bg_color, text_color) = if is_selected {
        (bg::SELECTED, text::BRIGHT)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .px(px(8.0))
        .py(px(4.0))
        .bg(bg_color)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(range.label().to_string()),
        )
}

/// Render the bar chart
fn render_chart_bars(data: &[EarningsDataPoint]) -> impl IntoElement {
    let max_sats = data.iter().map(|d| d.sats).max().unwrap_or(1);

    div()
        .h(px(140.0))
        .w_full()
        .flex()
        .items_end()
        .justify_around()
        .gap(px(4.0))
        .py(px(8.0))
        .children(data.iter().map(|point| {
            let height_ratio = point.sats as f32 / max_sats as f32;
            render_bar(&point.label, height_ratio, point.sats)
        }))
}

/// Render a single bar
fn render_bar(label: &str, height_ratio: f32, _sats: u64) -> impl IntoElement {
    let bar_height = (height_ratio * 100.0).max(4.0);

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        .flex_1()
        // Bar
        .child(
            div()
                .w_full()
                .max_w(px(40.0))
                .h(px(bar_height))
                .bg(accent::PRIMARY)
                .rounded_t(px(2.0))
                .cursor_pointer()
                .hover(|s| s.bg(accent::SECONDARY)),
        )
        // Label
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label.to_string()),
        )
}

/// Render chart footer with totals
fn render_chart_footer(total_sats: u64) -> impl IntoElement {
    div()
        .pt(px(8.0))
        .border_t_1()
        .border_color(border::SUBTLE)
        .flex()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("TOTAL"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_family(FONT_FAMILY)
                .text_color(theme_oa::status::SUCCESS)
                .child(format!("{} sats", format_sats(total_sats))),
        )
}

/// Format sats with thousands separators
fn format_sats(sats: u64) -> String {
    let s = sats.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().rev().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result.chars().rev().collect()
}

/// Generate mock earnings data for UI development
pub fn mock_earnings_data() -> Vec<EarningsDataPoint> {
    vec![
        EarningsDataPoint { label: "Mon".to_string(), sats: 850 },
        EarningsDataPoint { label: "Tue".to_string(), sats: 1200 },
        EarningsDataPoint { label: "Wed".to_string(), sats: 1450 },
        EarningsDataPoint { label: "Thu".to_string(), sats: 980 },
        EarningsDataPoint { label: "Fri".to_string(), sats: 1600 },
        EarningsDataPoint { label: "Sat".to_string(), sats: 1800 },
        EarningsDataPoint { label: "Sun".to_string(), sats: 1052 },
    ]
}
