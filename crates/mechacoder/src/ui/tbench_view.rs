//! TB2 view components for rendering Terminal-Bench entries.
//!
//! Bloomberg Terminal aesthetic: white on black, no emojis, monospace.

use crate::sdk_thread::{TBenchRunEntry, TBenchStreamEntry};
use gpui::{div, prelude::*, px, IntoElement, ParentElement, Styled};
use harbor::StreamEvent;
use terminalbench::TBRunStatus;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

/// View for TB2 run header
pub struct TBenchRunView {
    /// The run entry
    entry: TBenchRunEntry,
}

impl TBenchRunView {
    /// Create from a run entry
    pub fn from_entry(entry: &TBenchRunEntry) -> Self {
        Self {
            entry: entry.clone(),
        }
    }

    /// Get status indicator symbol and color
    fn status_indicator(&self) -> (&'static str, gpui::Hsla) {
        match self.entry.status {
            TBRunStatus::Queued => (".", text::MUTED),
            TBRunStatus::Running => ("*", status::RUNNING),
            TBRunStatus::Completed => {
                if self.entry.error.is_some() {
                    ("x", status::ERROR)
                } else {
                    ("o", status::SUCCESS)
                }
            }
            TBRunStatus::Error => ("!", status::ERROR),
        }
    }
}

impl IntoElement for TBenchRunView {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        let (symbol, color) = self.status_indicator();
        let progress = if self.entry.max_turns > 0 {
            format!(
                " - Turn {}/{}",
                self.entry.turns, self.entry.max_turns
            )
        } else {
            String::new()
        };

        div()
            .px(px(16.0))
            .py(px(12.0))
            .my(px(4.0))
            .mx(px(8.0))
            .bg(bg::CARD)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(4.0))
            .font_family(FONT_FAMILY)
            // Header row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    // Status symbol
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(color)
                            .child(symbol),
                    )
                    // TB2 label
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child("[TB2]"),
                    )
                    // Task name
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(text::PRIMARY)
                            .child(self.entry.task_name.clone()),
                    )
                    // Progress
                    .when(!progress.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(text::SECONDARY)
                                .child(progress),
                        )
                    }),
            )
            // Cost (if known)
            .when_some(self.entry.cost, |el, cost| {
                el.child(
                    div()
                        .mt(px(4.0))
                        .text_xs()
                        .text_color(text::MUTED)
                        .child(format!("${:.4}", cost)),
                )
            })
            // Error (if any)
            .when_some(self.entry.error.clone(), |el, error| {
                el.child(
                    div()
                        .mt(px(4.0))
                        .text_xs()
                        .text_color(status::ERROR)
                        .child(error),
                )
            })
    }
}

/// View for TB2 stream event
pub struct TBenchEventView {
    /// The stream entry
    entry: TBenchStreamEntry,
}

impl TBenchEventView {
    /// Create from a stream entry
    pub fn from_entry(entry: &TBenchStreamEntry) -> Self {
        Self {
            entry: entry.clone(),
        }
    }
}

impl IntoElement for TBenchEventView {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        match &self.entry.event {
            StreamEvent::RunStart {
                session_id,
                instruction,
            } => {
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(format!("[{}]", &session_id[..8.min(session_id.len())])),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text::SECONDARY)
                                    .child(instruction.chars().take(100).collect::<String>()),
                            ),
                    )
            }
            StreamEvent::Assistant { turn, text: content } => {
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .items_start()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .min_w(px(40.0))
                                    .child(format!("T{}", turn)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(text::PRIMARY)
                                    // Truncate long content
                                    .child(if content.len() > 500 {
                                        format!("{}...", &content[..500])
                                    } else {
                                        content.clone()
                                    }),
                            ),
                    )
            }
            StreamEvent::ToolUse { tool, id } => {
                div()
                    .px(px(16.0))
                    .py(px(4.0))
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(status::RUNNING),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::SECONDARY)
                                    .child(tool.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(format!("[{}]", &id[..8.min(id.len())])),
                            ),
                    )
            }
            StreamEvent::ToolResult { id, output, error } => {
                let (symbol, color, content) = if let Some(err) = error {
                    ("x", status::ERROR, err.chars().take(200).collect::<String>())
                } else if let Some(out) = output {
                    ("o", status::SUCCESS, out.chars().take(200).collect::<String>())
                } else {
                    ("o", status::SUCCESS, "(no output)".to_string())
                };

                div()
                    .px(px(16.0))
                    .py(px(4.0))
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .items_start()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(color)
                                    .child(symbol),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(format!("[{}]", &id[..8.min(id.len())])),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(text::SECONDARY)
                                    .child(content),
                            ),
                    )
            }
            StreamEvent::Complete {
                success,
                turns,
                cost,
                error,
            } => {
                let (symbol, color) = if *success {
                    ("o", status::SUCCESS)
                } else {
                    ("x", status::ERROR)
                };

                let cost_str = cost.map(|c| format!(" ${:.4}", c)).unwrap_or_default();

                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .my(px(4.0))
                    .font_family(FONT_FAMILY)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(color)
                                    .child(symbol),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text::PRIMARY)
                                    .child(if *success { "PASS" } else { "FAIL" }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(format!("{} turns{}", turns, cost_str)),
                            ),
                    )
                    .when_some(error.clone(), |el, err| {
                        el.child(
                            div()
                                .mt(px(4.0))
                                .text_xs()
                                .text_color(status::ERROR)
                                .child(err),
                        )
                    })
            }
        }
    }
}
