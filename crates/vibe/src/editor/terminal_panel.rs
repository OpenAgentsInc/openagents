//! Terminal panel component - OANIX terminal integration

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{TerminalLine, TerminalLineType};

/// Render the terminal panel
pub fn render_terminal_panel(lines: &[TerminalLine], input: &str) -> impl IntoElement {
    div()
        .id("terminal-panel")
        .h(px(180.0))
        .w_full()
        .flex()
        .flex_col()
        .bg(bg::APP)
        .border_t_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .h(px(28.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                // Tabs
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(render_terminal_tab("TERMINAL", true))
                        .child(render_terminal_tab("OUTPUT", false))
                        .child(render_terminal_tab("PROBLEMS", false)),
                )
                // Actions
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        // New terminal
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("+"),
                        )
                        // Clear
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("CLEAR"),
                        )
                        // Maximize
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("^"),
                        )
                        // Close
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("x"),
                        ),
                ),
        )
        // Terminal content
        .child(
            div()
                .id("terminal-output")
                .flex_1()
                .overflow_y_scroll()
                .p(px(8.0))
                .children(lines.iter().map(|line| {
                    render_terminal_line(line)
                })),
        )
        // Input line
        .child(
            div()
                .h(px(28.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(8.0))
                .bg(bg::ELEVATED)
                .border_t_1()
                .border_color(border::DEFAULT)
                // Prompt
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::SUCCESS)
                        .mr(px(4.0))
                        .child("/workspace $"),
                )
                // Input
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(if input.is_empty() {
                            "_".to_string()
                        } else {
                            format!("{}_", input)
                        }),
                ),
        )
}

/// Render a terminal tab
fn render_terminal_tab(label: &str, is_active: bool) -> impl IntoElement {
    let label = label.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .px(px(8.0))
        .py(px(4.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(label),
        )
}

/// Render a single terminal line
fn render_terminal_line(line: &TerminalLine) -> impl IntoElement {
    let (prefix, color) = match line.line_type {
        TerminalLineType::Input => ("$ ", status::SUCCESS),
        TerminalLineType::Output => ("", text::PRIMARY),
        TerminalLineType::Error => ("", status::ERROR),
        TerminalLineType::System => ("# ", Hsla { h: 0.6, s: 0.6, l: 0.6, a: 1.0 }),
    };

    div()
        .w_full()
        .flex()
        .items_start()
        .py(px(1.0))
        .when(!prefix.is_empty(), |el| {
            el.child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(color)
                    .child(prefix),
            )
        })
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(line.content.clone()),
        )
}
