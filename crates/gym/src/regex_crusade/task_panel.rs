//! Task Info Panel - Shows regex-log task and current best solution

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{CrusadeSession, CrusadeStatus, REGEX_LOG_DESCRIPTION, REGEX_LOG_TASK_NAME};

/// Task info panel component
pub struct TaskPanel {
    session: CrusadeSession,
    focus_handle: FocusHandle,
}

impl TaskPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            session: CrusadeSession::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_session(&mut self, session: CrusadeSession, cx: &mut Context<Self>) {
        self.session = session;
        cx.notify();
    }

    fn render_task_header(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Task name with target badge
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(REGEX_LOG_TASK_NAME),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .bg(status::ERROR_BG)
                            .rounded(px(4.0))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::ERROR)
                            .font_weight(FontWeight::BOLD)
                            .child("TARGET: 100%"),
                    ),
            )
            // Status indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(self.render_status_badge())
                    .when_some(self.session.best_regex.as_ref(), |el, _| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::DISABLED)
                                .child(format!("{} iterations", self.session.iterations.len())),
                        )
                    }),
            )
    }

    fn render_status_badge(&self) -> impl IntoElement {
        let (bg_color, text_color) = match self.session.status {
            CrusadeStatus::Idle => (bg::ELEVATED, text::MUTED),
            CrusadeStatus::GeneratingTests | CrusadeStatus::RunningIteration => {
                (status::INFO_BG, status::RUNNING)
            }
            CrusadeStatus::Validating => (status::WARNING_BG, status::WARNING),
            CrusadeStatus::Completed => (status::SUCCESS_BG, status::SUCCESS),
            CrusadeStatus::Failed => (status::ERROR_BG, status::ERROR),
        };

        div()
            .px(px(8.0))
            .py(px(3.0))
            .bg(bg_color)
            .rounded(px(4.0))
            .text_size(px(10.0))
            .font_family(FONT_FAMILY)
            .text_color(text_color)
            .font_weight(FontWeight::MEDIUM)
            .child(self.session.status.label())
    }

    fn render_pass_rate(&self) -> impl IntoElement {
        let pass_rate = if self.session.tests_total == 0 {
            0.0
        } else {
            self.session.tests_passed as f32 / self.session.tests_total as f32
        };
        let progress_width = (pass_rate * 180.0).max(4.0);
        let is_complete = pass_rate >= 1.0;

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Large pass rate display
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(36.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_complete {
                                status::SUCCESS
                            } else if pass_rate > 0.8 {
                                status::WARNING
                            } else {
                                text::PRIMARY
                            })
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{:.0}%", pass_rate * 100.0)),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!(
                                "{}/{}",
                                self.session.tests_passed, self.session.tests_total
                            )),
                    ),
            )
            // Progress bar
            .child(
                div()
                    .w(px(180.0))
                    .h(px(8.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(progress_width))
                            .h_full()
                            .bg(if is_complete {
                                status::SUCCESS
                            } else {
                                status::INFO
                            })
                            .rounded(px(4.0)),
                    ),
            )
    }

    fn render_test_quality_summary(&self) -> impl IntoElement {
        let stub_count = self.session.stub_count;
        let real_count = self.session.real_count;

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Test Quality"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    // Real tests (good)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(status::SUCCESS)
                                    .child("R"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .child(format!("{} real", real_count)),
                            ),
                    )
                    // Stub tests (bad)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(status::ERROR)
                                    .child("S"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(if stub_count > 0 {
                                        status::ERROR
                                    } else {
                                        text::PRIMARY
                                    })
                                    .child(format!("{} stubs", stub_count)),
                            ),
                    ),
            )
            // Warning if many stubs
            .when(stub_count > real_count && (stub_count + real_count) > 0, |el| {
                el.child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(status::WARNING_BG)
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::WARNING)
                        .child("More stubs than real tests!"),
                )
            })
    }

    fn render_current_regex(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Current Best Regex"),
            )
            .child(
                div()
                    .p(px(10.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(border::SUBTLE)
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(
                                self.session
                                    .best_regex
                                    .clone()
                                    .unwrap_or_else(|| "No solution yet".to_string()),
                            ),
                    ),
            )
    }

    fn render_task_description(&self) -> impl IntoElement {
        div()
            .id("task-description-scroll")
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .flex_1()
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Task Description"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::SECONDARY)
                    .line_height(px(16.0))
                    .child(REGEX_LOG_DESCRIPTION),
            )
    }
}

impl Focusable for TaskPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            .child(self.render_task_header())
            .child(self.render_pass_rate())
            .child(self.render_test_quality_summary())
            .child(self.render_current_regex())
            .child(self.render_task_description())
    }
}
