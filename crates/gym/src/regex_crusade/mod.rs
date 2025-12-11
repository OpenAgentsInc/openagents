//! RegexCrusade - Laser-focused UI for solving regex-log
//!
//! Single-purpose screen for hitting 100% on the regex-log Terminal-Bench task.

pub mod types;
pub mod task_panel;
pub mod test_panel;
pub mod iteration_log;

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use self::task_panel::TaskPanel;
use self::test_panel::TestPanel;
use self::iteration_log::IterationLog;
use self::types::{
    sample_iterations, sample_tests, CrusadeSession, CrusadeStatus, CrusadeTest, Iteration,
    TestQuality,
};

/// Main RegexCrusade screen
pub struct RegexCrusadeScreen {
    /// Task info panel (left)
    task_panel: Entity<TaskPanel>,
    /// Test list panel (center)
    test_panel: Entity<TestPanel>,
    /// Iteration log (right)
    iteration_log: Entity<IterationLog>,
    /// Current session state
    session: CrusadeSession,
    /// Focus handle
    focus_handle: FocusHandle,
}

impl RegexCrusadeScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let task_panel = cx.new(|cx| TaskPanel::new(cx));
        let test_panel = cx.new(|cx| TestPanel::new(cx));
        let iteration_log = cx.new(|cx| IterationLog::new(cx));

        // Initialize with sample data for MVP
        let sample_test_data = sample_tests();
        let sample_iteration_data = sample_iterations();

        let stub_count = sample_test_data
            .iter()
            .filter(|t| t.quality == TestQuality::Stub)
            .count() as u32;
        let real_count = sample_test_data
            .iter()
            .filter(|t| t.quality == TestQuality::Real)
            .count() as u32;
        let tests_passed = sample_test_data
            .iter()
            .filter(|t| t.status == types::TestRunStatus::Passed)
            .count() as u32;
        let tests_total = sample_test_data.len() as u32;

        let session = CrusadeSession {
            status: CrusadeStatus::Idle,
            best_regex: sample_iteration_data.last().map(|i| i.regex_pattern.clone()),
            tests_passed,
            tests_total,
            stub_count,
            real_count,
            iterations: sample_iteration_data.clone(),
        };

        // Update child panels with sample data
        task_panel.update(cx, |panel, cx| {
            panel.set_session(session.clone(), cx);
        });
        test_panel.update(cx, |panel, cx| {
            panel.set_tests(sample_test_data, cx);
        });
        iteration_log.update(cx, |log, cx| {
            log.set_iterations(sample_iteration_data, cx);
        });

        Self {
            task_panel,
            test_panel,
            iteration_log,
            session,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Update session and propagate to child panels
    #[allow(dead_code)]
    pub fn update_session(&mut self, session: CrusadeSession, cx: &mut Context<Self>) {
        self.session = session.clone();
        self.task_panel.update(cx, |panel, cx| {
            panel.set_session(session.clone(), cx);
        });
        self.iteration_log.update(cx, |log, cx| {
            log.set_iterations(session.iterations, cx);
        });
        cx.notify();
    }

    /// Add tests from generation
    #[allow(dead_code)]
    pub fn set_tests(&mut self, tests: Vec<CrusadeTest>, cx: &mut Context<Self>) {
        let stub_count = tests
            .iter()
            .filter(|t| t.quality == TestQuality::Stub)
            .count() as u32;
        let real_count = tests
            .iter()
            .filter(|t| t.quality == TestQuality::Real)
            .count() as u32;

        self.session.stub_count = stub_count;
        self.session.real_count = real_count;
        self.session.tests_total = tests.len() as u32;

        self.test_panel.update(cx, |panel, cx| {
            panel.set_tests(tests, cx);
        });
        self.task_panel.update(cx, |panel, cx| {
            panel.set_session(self.session.clone(), cx);
        });
        cx.notify();
    }

    /// Add a new iteration result
    #[allow(dead_code)]
    pub fn add_iteration(&mut self, iteration: Iteration, cx: &mut Context<Self>) {
        self.session.iterations.push(iteration.clone());
        self.session.tests_passed = iteration.passed;
        self.session.best_regex = Some(iteration.regex_pattern.clone());

        self.iteration_log.update(cx, |log, cx| {
            log.add_iteration(iteration, cx);
        });
        self.task_panel.update(cx, |panel, cx| {
            panel.set_session(self.session.clone(), cx);
        });
        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_generate = !self.session.status.is_busy();
        let can_validate = !self.session.status.is_busy() && self.session.best_regex.is_some();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            // Left: Title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("RegexCrusade"),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .bg(status::ERROR_BG)
                            .rounded(px(4.0))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::ERROR)
                            .font_weight(FontWeight::BOLD)
                            .child("regex-log"),
                    ),
            )
            // Right: Action buttons
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Generate Tests button
                    .child(
                        div()
                            .id("generate-tests-btn")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .cursor(if can_generate {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .bg(if can_generate {
                                status::INFO
                            } else {
                                bg::ELEVATED
                            })
                            .text_color(if can_generate {
                                text::BRIGHT
                            } else {
                                text::DISABLED
                            })
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .font_weight(FontWeight::MEDIUM)
                            .when(can_generate, |el| {
                                el.on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_this, _evt, _window, _cx| {
                                        // TODO: Start test generation
                                    }),
                                )
                            })
                            .child("Generate Tests"),
                    )
                    // Validate button
                    .child(
                        div()
                            .id("validate-btn")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .cursor(if can_validate {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::default()
                            })
                            .bg(if can_validate {
                                status::SUCCESS
                            } else {
                                bg::ELEVATED
                            })
                            .text_color(if can_validate {
                                text::BRIGHT
                            } else {
                                text::DISABLED
                            })
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .font_weight(FontWeight::MEDIUM)
                            .when(can_validate, |el| {
                                el.on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_this, _evt, _window, _cx| {
                                        // TODO: Run validation
                                    }),
                                )
                            })
                            .child("Validate Current"),
                    ),
            )
    }
}

impl Focusable for RegexCrusadeScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RegexCrusadeScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Header with action buttons
            .child(self.render_header(cx))
            // Three-panel layout
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Left: Task Panel (narrow)
                    .child(
                        div()
                            .w(px(260.0))
                            .h_full()
                            .border_r_1()
                            .border_color(border::DEFAULT)
                            .child(self.task_panel.clone()),
                    )
                    // Center: Test Panel (main)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .border_r_1()
                            .border_color(border::DEFAULT)
                            .child(self.test_panel.clone()),
                    )
                    // Right: Iteration Log
                    .child(
                        div()
                            .w(px(320.0))
                            .h_full()
                            .child(self.iteration_log.clone()),
                    ),
            )
    }
}
