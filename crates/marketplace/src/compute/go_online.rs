//! Go Online toggle component - Big button to start selling compute

use gpui::*;
use theme::{bg, border, text, status, FONT_FAMILY};

/// Render the Go Online panel
pub fn render_go_online_panel(is_online: bool, models_configured: u32) -> impl IntoElement {
    div()
        .w_full()
        .p(px(24.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(16.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(12.0))
        // Big toggle button
        .child(render_toggle_button(is_online))
        // Status info
        .child(render_status_info(is_online, models_configured))
}

/// Render the big toggle button
fn render_toggle_button(is_online: bool) -> impl IntoElement {
    let (button_bg, button_border, text_color, icon) = if is_online {
        (status::SUCCESS_BG, status::SUCCESS, status::SUCCESS, "●")
    } else {
        (bg::ELEVATED, border::SUBTLE, text::PRIMARY, "○")
    };

    div()
        .w(px(200.0))
        .h(px(60.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .bg(button_bg)
        .border_2()
        .border_color(button_border)
        .rounded(px(30.0))
        .cursor_pointer()
        .hover(|s| {
            if is_online {
                s.bg(status::ERROR_BG).border_color(status::ERROR)
            } else {
                s.bg(status::SUCCESS_BG).border_color(status::SUCCESS)
            }
        })
        // Status dot
        .child(
            div()
                .text_size(px(16.0))
                .text_color(text_color)
                .child(icon.to_string()),
        )
        // Button text
        .child(
            div()
                .text_size(px(16.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(if is_online {
                    "ONLINE"
                } else {
                    "GO ONLINE"
                }.to_string()),
        )
}

/// Render status information
fn render_status_info(is_online: bool, models_configured: u32) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(24.0))
        // Status
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("STATUS:"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(if is_online { status::SUCCESS } else { text::DISABLED })
                        .child(if is_online { "EARNING" } else { "OFFLINE" }.to_string()),
                ),
        )
        // Models configured
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("MODELS:"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(format!("{} configured", models_configured)),
                ),
        )
}

/// Render the earning rate when online
pub fn render_earning_rate(sats_per_hour: u64) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .child("⚡"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("EARNING"),
        )
        .child(
            div()
                .text_size(px(16.0))
                .font_family(FONT_FAMILY)
                .text_color(status::SUCCESS)
                .child(format!("{} sats/hr", sats_per_hour)),
        )
}
