//! DVM list component - Data Vending Machine listings

use gpui::*;
use theme::{bg, border, text, accent, FONT_FAMILY};

use crate::types::{DVMListing, PricingUnit};

/// Render the trending DVMs section
pub fn render_dvm_list(dvms: &[DVMListing]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        .overflow_hidden()
        // Header
        .child(render_section_header())
        // DVM rows
        .children(dvms.iter().map(|dvm| {
            render_dvm_row(dvm)
        }))
}

/// Render the section header
fn render_section_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .bg(bg::HEADER)
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(14.0))
                .child("🔥"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("POPULAR DATA VENDING MACHINES"),
        )
}

/// Render a DVM row
fn render_dvm_row(dvm: &DVMListing) -> impl IntoElement {
    let usage_percent = (dvm.request_count as f32 / 10_000.0).min(1.0);

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(border::SUBTLE)
        .cursor_pointer()
        .hover(|s| s.bg(bg::ROW))
        // Top row: icon, name, pricing
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(16.0))
                                .child(get_dvm_icon(dvm.kind)),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(dvm.name.clone()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::PRIMARY)
                        .child(format!("{} sats{}", dvm.sats_per_unit, dvm.pricing_unit.label())),
                ),
        )
        // Middle row: usage bar
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .h(px(6.0))
                        .bg(bg::ELEVATED)
                        .rounded(px(3.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(usage_percent))
                                .bg(accent::PRIMARY)
                                .rounded(px(3.0)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} req", format_k(dvm.request_count))),
                ),
        )
        // Bottom row: provider, rating, action
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(dvm.provider_name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .child("⭐"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(theme::status::WARNING)
                                        .child(format!("{:.1}", dvm.rating)),
                                ),
                        )
                        .child(render_use_button()),
                ),
        )
}

/// Render the USE button
fn render_use_button() -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(4.0))
        .bg(accent::PRIMARY_MUTED)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(accent::PRIMARY))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(accent::PRIMARY)
                .child("USE"),
        )
}

/// Get icon for DVM kind
fn get_dvm_icon(kind: u32) -> &'static str {
    match kind {
        5000..=5099 => "🎙️", // Transcription
        5100..=5199 => "🖼️", // Image
        5200..=5299 => "🤖", // Inference
        5300..=5399 => "📝", // Translation
        _ => "🔧",
    }
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

/// Generate mock DVMs for UI development
pub fn mock_dvms() -> Vec<DVMListing> {
    vec![
        DVMListing::mock("Whisper Transcription", 5000, 50, PricingUnit::PerMinute),
        DVMListing::mock("GPT-4 Vision Analysis", 5100, 200, PricingUnit::PerImage),
        DVMListing::mock("DeepSeek R1 Inference", 5200, 5, PricingUnit::Per1KTokens),
        DVMListing::mock("Claude Translation", 5300, 10, PricingUnit::Per1KTokens),
    ]
}
