//! RegexCrusade - Laser-focused UI for solving regex-log
//!
//! Single-purpose screen for hitting 100% on the regex-log Terminal-Bench task.

pub mod iteration_log;
pub mod task_panel;
pub mod test_panel;
pub mod types;

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use theme::{bg, border, status, text, FONT_FAMILY};
use tokio::sync::mpsc;

use self::iteration_log::IterationLog;
use self::task_panel::TaskPanel;
use self::test_panel::TestPanel;
use self::types::{
    detect_stub, CrusadeCategory, CrusadeSession, CrusadeStatus, CrusadeTest, TestQuality,
    TestRunStatus, REGEX_LOG_DESCRIPTION, REGEX_LOG_TASK_ID,
};
use crate::testgen::service::{GenerationRequest, TestGenEvent, TestGenService};
use testgen::TestGenContext;

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
    /// TestGen service for background generation
    testgen_service: Arc<TestGenService>,
    /// Event receiver from TestGen
    event_receiver: Option<mpsc::UnboundedReceiver<TestGenEvent>>,
    /// Focus handle
    focus_handle: FocusHandle,
}

impl RegexCrusadeScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let task_panel = cx.new(|cx| TaskPanel::new(cx));
        let test_panel = cx.new(|cx| TestPanel::new(cx));
        let iteration_log = cx.new(|cx| IterationLog::new(cx));

        // Start with empty state - no sample data
        let session = CrusadeSession::default();

        // Update child panels with empty state
        task_panel.update(cx, |panel, cx| {
            panel.set_session(session.clone(), cx);
        });

        Self {
            task_panel,
            test_panel,
            iteration_log,
            session,
            testgen_service: Arc::new(TestGenService::new()),
            event_receiver: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Start test generation
    fn start_generation(&mut self, cx: &mut Context<Self>) {
        if self.testgen_service.is_generating() {
            return;
        }

        // Clear current tests
        self.session = CrusadeSession {
            status: CrusadeStatus::GeneratingTests,
            ..Default::default()
        };

        self.test_panel.update(cx, |panel, cx| {
            panel.set_tests(vec![], cx);
        });
        self.task_panel.update(cx, |panel, cx| {
            panel.set_session(self.session.clone(), cx);
        });

        // Start generation
        let request = GenerationRequest {
            task_id: REGEX_LOG_TASK_ID.to_string(),
            task_description: REGEX_LOG_DESCRIPTION.to_string(),
            context: TestGenContext::Benchmark,
        };

        let receiver = self.testgen_service.start_generation(request);
        self.event_receiver = Some(receiver);

        cx.notify();
    }

    /// Poll for TestGen events and update UI
    fn poll_events(&mut self, cx: &mut Context<Self>) {
        // Collect events first to avoid borrow issues
        let events: Vec<TestGenEvent> = {
            let Some(ref mut receiver) = self.event_receiver else {
                return;
            };
            let mut collected = vec![];
            while let Ok(event) = receiver.try_recv() {
                collected.push(event);
            }
            collected
        };

        if events.is_empty() {
            return;
        }

        let mut tests_updated = false;
        let mut new_tests: Vec<CrusadeTest> = vec![];
        let mut should_clear_receiver = false;

        // Process collected events
        for event in events {
            match event {
                TestGenEvent::Progress {
                    phase,
                    category,
                    round,
                    status: status_msg,
                } => {
                    // Update status message if needed
                    eprintln!(
                        "[TestGen] {} {:?} round {} - {}",
                        phase, category, round, status_msg
                    );
                }
                TestGenEvent::TestGenerated(test) => {
                    // Convert testgen::GeneratedTest to CrusadeTest
                    let crusade_test = convert_test(&test);
                    new_tests.push(crusade_test);
                    tests_updated = true;
                }
                TestGenEvent::Reflection(entry) => {
                    eprintln!("[TestGen] Reflection: {:?}", entry.action);
                }
                TestGenEvent::Complete {
                    total_tests,
                    total_rounds,
                    duration_ms,
                } => {
                    eprintln!(
                        "[TestGen] Complete: {} tests, {} rounds, {}ms",
                        total_tests, total_rounds, duration_ms
                    );
                    self.session.status = CrusadeStatus::Idle;
                    should_clear_receiver = true;
                }
                TestGenEvent::Error(err) => {
                    eprintln!("[TestGen] Error: {}", err);
                    self.session.status = CrusadeStatus::Failed;
                    should_clear_receiver = true;
                }
            }
        }

        if should_clear_receiver {
            self.event_receiver = None;
        }

        // Update tests if we got new ones
        if tests_updated {
            // Get existing tests and append new ones
            let mut all_tests = vec![];
            self.test_panel.update(cx, |_panel, _cx| {
                // Panel stores tests internally, but we'll just use new_tests for now
            });
            all_tests.extend(new_tests);

            // Update counts
            let stub_count = all_tests
                .iter()
                .filter(|t| t.quality == TestQuality::Stub)
                .count() as u32;
            let real_count = all_tests
                .iter()
                .filter(|t| t.quality == TestQuality::Real)
                .count() as u32;

            self.session.stub_count = stub_count;
            self.session.real_count = real_count;
            self.session.tests_total = all_tests.len() as u32;

            self.test_panel.update(cx, |panel, cx| {
                panel.set_tests(all_tests, cx);
            });
            self.task_panel.update(cx, |panel, cx| {
                panel.set_session(self.session.clone(), cx);
            });
        }

        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_generate = !self.session.status.is_busy();
        let can_validate = !self.session.status.is_busy() && self.session.best_regex.is_some();
        let is_generating = self.session.status == CrusadeStatus::GeneratingTests;

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
                            .bg(if is_generating {
                                status::WARNING_BG
                            } else if can_generate {
                                status::INFO
                            } else {
                                bg::ELEVATED
                            })
                            .text_color(if is_generating {
                                status::WARNING
                            } else if can_generate {
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
                                    cx.listener(|this, _evt, _window, cx| {
                                        this.start_generation(cx);
                                    }),
                                )
                            })
                            .child(if is_generating {
                                "Generating..."
                            } else {
                                "Generate Tests"
                            }),
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
        // Poll for events if generation is in progress
        if self.event_receiver.is_some() {
            self.poll_events(cx);
        }

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

/// Convert testgen::GeneratedTest to CrusadeTest
fn convert_test(test: &testgen::GeneratedTest) -> CrusadeTest {
    let category = match test.category {
        testgen::TestCategory::AntiCheat => CrusadeCategory::AntiCheat,
        testgen::TestCategory::Existence => CrusadeCategory::Existence,
        testgen::TestCategory::Correctness => CrusadeCategory::Correctness,
        testgen::TestCategory::Boundary => CrusadeCategory::Boundary,
        testgen::TestCategory::Integration => CrusadeCategory::Integration,
        _ => CrusadeCategory::Correctness, // Map legacy categories to Correctness
    };

    // For now we don't have the actual test code, so we'll synthesize it
    let code = format!(
        "# {}\n# Input: {}\n# Expected: {:?}\npass  # TODO: implement",
        test.reasoning,
        test.input,
        test.expected_output
    );

    let quality = detect_stub(&code);

    CrusadeTest {
        id: test.id.clone(),
        category,
        quality,
        status: TestRunStatus::NotRun,
        input: test.input.clone(),
        expected: test.expected_output.clone(),
        actual: None,
        code,
        reasoning: test.reasoning.clone(),
        confidence: test.confidence as f32,
    }
}
