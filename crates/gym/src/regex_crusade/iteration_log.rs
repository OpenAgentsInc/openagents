//! Streaming Log - Real-time FM activity display
//!
//! Shows everything happening as it happens: prompts, responses, tests generated.

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{LogEntry, LogEntryKind};

/// Streaming log panel component
pub struct IterationLog {
    log_entries: Vec<LogEntry>,
    focus_handle: FocusHandle,
}

impl IterationLog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            log_entries: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Add a log entry
    pub fn add_log_entry(&mut self, entry: LogEntry, cx: &mut Context<Self>) {
        self.log_entries.push(entry);
        cx.notify();
    }

    /// Clear all logs
    pub fn clear_logs(&mut self, cx: &mut Context<Self>) {
        self.log_entries.clear();
        cx.notify();
    }

    /// Legacy method for compatibility
    pub fn set_iterations(&mut self, _iterations: Vec<super::types::Iteration>, cx: &mut Context<Self>) {
        // Not used in streaming mode
        cx.notify();
    }

    /// Legacy method for compatibility
    pub fn add_iteration(&mut self, _iteration: super::types::Iteration, cx: &mut Context<Self>) {
        // Not used in streaming mode
        cx.notify();
    }

    fn render_log_entry(&self, entry: &LogEntry, idx: usize) -> impl IntoElement {
        let is_last = idx == self.log_entries.len() - 1;
        let time_str = entry.timestamp.format("%H:%M:%S").to_string();

        // Color coding by entry kind
        let (badge_bg, badge_text, msg_color) = match entry.kind {
            LogEntryKind::Info => (bg::ELEVATED, text::MUTED, text::SECONDARY),
            LogEntryKind::Progress => (status::INFO_BG, status::INFO, text::SECONDARY),
            LogEntryKind::Prompt => (status::WARNING_BG, status::WARNING, text::PRIMARY),
            LogEntryKind::Response => (status::SUCCESS_BG, status::SUCCESS, text::PRIMARY),
            LogEntryKind::TestGenerated => (status::SUCCESS_BG, status::SUCCESS, text::BRIGHT),
            LogEntryKind::Reflection => (bg::ELEVATED, text::MUTED, text::SECONDARY),
            LogEntryKind::Complete => (status::SUCCESS_BG, status::SUCCESS, text::BRIGHT),
            LogEntryKind::Error => (status::ERROR_BG, status::ERROR, status::ERROR),
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
    }

    fn render_header(&self) -> impl IntoElement {
        let count = self.log_entries.len();
        let test_count = self
            .log_entries
            .iter()
            .filter(|e| e.kind == LogEntryKind::TestGenerated)
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
                    .child("Streaming Log"),
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
                    .when(test_count > 0, |el| {
                        el.child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(status::SUCCESS_BG)
                                .rounded(px(3.0))
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(status::SUCCESS)
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{} tests", test_count)),
                        )
                    }),
            )
    }
}

impl Focusable for IterationLog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for IterationLog {
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
            // Log entries (scrollable, most recent at bottom, auto-scroll)
            .child(
                div()
                    .id("streaming-log-scroll")
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
                                            .child("Click 'Generate Tests' to start"),
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
