//! Streaming Log Panel - Real-time activity display
//!
//! Shows everything happening as it happens: tool uses, progress, results.

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{LogEntry, LogKind};

/// Streaming log panel component
pub struct LogPanel {
    log_entries: Vec<LogEntry>,
    focus_handle: FocusHandle,
}

impl LogPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            log_entries: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Add a log entry
    pub fn add_entry(&mut self, entry: LogEntry, cx: &mut Context<Self>) {
        self.log_entries.push(entry);
        cx.notify();
    }

    /// Clear all logs
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.log_entries.clear();
        cx.notify();
    }

    fn render_log_entry(&self, entry: &LogEntry, idx: usize) -> impl IntoElement {
        let is_last = idx == self.log_entries.len() - 1;
        let time_str = entry.timestamp.format("%H:%M:%S").to_string();

        // Color coding by entry kind
        let (badge_bg, badge_text, msg_color) = match entry.kind {
            LogKind::Info => (bg::ELEVATED, text::MUTED, text::SECONDARY),
            LogKind::Progress => (status::INFO_BG, status::INFO, text::SECONDARY),
            LogKind::Tool => (status::WARNING_BG, status::WARNING, text::PRIMARY),
            LogKind::Thinking => (bg::ELEVATED, text::MUTED, text::SECONDARY),
            LogKind::TestResult => (status::INFO_BG, status::INFO, text::PRIMARY),
            LogKind::Success => (status::SUCCESS_BG, status::SUCCESS, text::BRIGHT),
            LogKind::Error => (status::ERROR_BG, status::ERROR, status::ERROR),
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(if is_last { bg::SELECTED } else { bg::ROW })
            .when(!is_last, |el| {
                el.border_b_1().border_color(border::SUBTLE)
            })
            // Header row: timestamp + badge
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Timestamp
                    .child(
                        div()
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(time_str),
                    )
                    // Kind badge
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(badge_bg)
                            .rounded(px(3.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(badge_text)
                                            .child(entry.kind.icon()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(8.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(badge_text)
                                            .font_weight(FontWeight::BOLD)
                                            .child(entry.kind.label()),
                                    ),
                            ),
                    ),
            )
            // Message content
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(msg_color)
                    .line_height(px(16.0))
                    .child(entry.message.clone()),
            )
            // Optional details (collapsible)
            .when_some(entry.details.as_ref(), |el, details| {
                el.child(
                    div()
                        .mt(px(4.0))
                        .p(px(8.0))
                        .bg(bg::ELEVATED)
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .max_h(px(100.0))
                        .overflow_hidden()
                        .child(details.clone()),
                )
            })
    }

    fn render_header(&self) -> impl IntoElement {
        let count = self.log_entries.len();
        let tool_count = self
            .log_entries
            .iter()
            .filter(|e| e.kind == LogKind::Tool)
            .count();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::ELEVATED)
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Activity Log"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!("{} events", count)),
                    )
                    .when(tool_count > 0, |el| {
                        el.child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(status::WARNING_BG)
                                .rounded(px(3.0))
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(status::WARNING)
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{} tools", tool_count)),
                        )
                    }),
            )
    }
}

impl Focusable for LogPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LogPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let entries_clone = self.log_entries.clone();

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            // Header
            .child(self.render_header())
            // Log entries (scrollable)
            .child(
                div()
                    .id("activity-log-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(entries_clone.is_empty(), |el| {
                        el.flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("No activity yet"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::DISABLED)
                                            .child("Click 'Start' to begin"),
                                    ),
                            )
                    })
                    .when(!entries_clone.is_empty(), |el| {
                        el.children(
                            entries_clone
                                .iter()
                                .enumerate()
                                .map(|(idx, entry)| self.render_log_entry(entry, idx)),
                        )
                    }),
            )
    }
}
