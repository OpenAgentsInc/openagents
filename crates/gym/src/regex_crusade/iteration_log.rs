//! Iteration Log - Turn-by-turn history with pass rate sparkline

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::types::Iteration;

/// Iteration log panel component
pub struct IterationLog {
    iterations: Vec<Iteration>,
    focus_handle: FocusHandle,
}

impl IterationLog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            iterations: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_iterations(&mut self, iterations: Vec<Iteration>, cx: &mut Context<Self>) {
        self.iterations = iterations;
        cx.notify();
    }

    pub fn add_iteration(&mut self, iteration: Iteration, cx: &mut Context<Self>) {
        self.iterations.push(iteration);
        cx.notify();
    }

    fn render_sparkline(&self) -> impl IntoElement {
        // Sparkline showing pass rate over iterations
        let points: Vec<f32> = self.iterations.iter().map(|i| i.pass_rate()).collect();
        let max_points = 20;
        let recent_points: Vec<f32> = if points.len() > max_points {
            points[points.len() - max_points..].to_vec()
        } else {
            points.clone()
        };

        let width = 280.0;
        let height = 50.0;
        let point_width = if recent_points.is_empty() {
            0.0
        } else {
            width / recent_points.len() as f32
        };

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
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .font_weight(FontWeight::MEDIUM)
                            .child("Pass Rate Trend"),
                    )
                    .when(!recent_points.is_empty(), |el| {
                        let latest = recent_points.last().copied().unwrap_or(0.0);
                        el.child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .text_color(if latest >= 1.0 {
                                    status::SUCCESS
                                } else {
                                    text::PRIMARY
                                })
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("{:.0}%", latest * 100.0)),
                        )
                    }),
            )
            // Sparkline container
            .child(
                div()
                    .w(px(width))
                    .h(px(height))
                    .bg(bg::ELEVATED)
                    .rounded(px(4.0))
                    .flex()
                    .items_end()
                    .gap(px(1.0))
                    .overflow_hidden()
                    .when(recent_points.is_empty(), |el| {
                        el.items_center().justify_center().child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::DISABLED)
                                .child("No iterations yet"),
                        )
                    })
                    .when(!recent_points.is_empty(), |el| {
                        el.children(recent_points.iter().map(|rate| {
                            let bar_height = (rate * height).max(2.0);
                            let color = if *rate >= 1.0 {
                                status::SUCCESS
                            } else if *rate >= 0.8 {
                                status::WARNING
                            } else {
                                status::INFO
                            };

                            div()
                                .w(px(point_width - 1.0))
                                .h(px(bar_height))
                                .bg(color)
                                .rounded_t(px(1.0))
                        }))
                    }),
            )
    }

    fn render_iteration_entry(&self, iteration: &Iteration, idx: usize) -> impl IntoElement {
        let is_improvement = if idx > 0 {
            let prev_idx = idx - 1;
            if prev_idx < self.iterations.len() {
                iteration.pass_rate() > self.iterations[prev_idx].pass_rate()
            } else {
                false
            }
        } else {
            false
        };

        let is_complete = iteration.passed == iteration.total && iteration.total > 0;
        let is_last = idx == self.iterations.len() - 1;

        div()
            .flex()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(10.0))
            .bg(bg::ROW)
            .when(!is_last, |el| {
                el.border_b_1().border_color(border::SUBTLE)
            })
            // Turn number
            .child(
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(6.0))
                    .bg(if is_complete {
                        status::SUCCESS_BG
                    } else {
                        bg::ELEVATED
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_complete {
                                status::SUCCESS
                            } else {
                                text::MUTED
                            })
                            .font_weight(FontWeight::BOLD)
                            .child(format!("#{}", iteration.turn)),
                    ),
            )
            // Content
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .overflow_hidden()
                    // Pass rate + change indicator
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(if is_complete {
                                        status::SUCCESS
                                    } else {
                                        text::PRIMARY
                                    })
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!(
                                        "{}/{} ({:.0}%)",
                                        iteration.passed,
                                        iteration.total,
                                        iteration.pass_rate() * 100.0
                                    )),
                            )
                            .when(is_improvement, |el| {
                                el.child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(status::SUCCESS)
                                        .child("+"),
                                )
                            }),
                    )
                    // Change description
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .line_height(px(15.0))
                            .text_ellipsis()
                            .child(iteration.change_description.clone()),
                    )
                    // Regex preview
                    .child(
                        div()
                            .mt(px(4.0))
                            .p(px(6.0))
                            .bg(bg::CODE)
                            .rounded(px(4.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .text_ellipsis()
                                    .child(iteration.regex_pattern.clone()),
                            ),
                    ),
            )
            // Duration
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(format!("{}ms", iteration.duration_ms)),
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
        let iteration_count = self.iterations.len();
        let iterations_clone = self.iterations.clone();

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            // Sparkline
            .child(self.render_sparkline())
            // Header
            .child(
                div()
                    .px(px(16.0))
                    .py(px(10.0))
                    .border_b_1()
                    .border_color(border::SUBTLE)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .font_weight(FontWeight::MEDIUM)
                            .child("Iteration History"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!("{} turns", iteration_count)),
                    ),
            )
            // Iteration list (scrollable, most recent first)
            .child(
                div()
                    .id("iteration-log-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(iterations_clone.is_empty(), |el| {
                        el.flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("No iterations yet"),
                            )
                    })
                    .when(!iterations_clone.is_empty(), |el| {
                        // Show most recent first
                        el.children(
                            iterations_clone
                                .iter()
                                .enumerate()
                                .rev()
                                .map(|(idx, iteration)| self.render_iteration_entry(iteration, idx)),
                        )
                    }),
            )
    }
}
