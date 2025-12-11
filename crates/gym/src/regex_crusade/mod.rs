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
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use tokio::sync::mpsc;

use self::iteration_log::IterationLog;
use self::task_panel::TaskPanel;
use self::test_panel::TestPanel;
use self::types::{
    CrusadeCategory, CrusadeSession, CrusadeStatus, CrusadeTest, LogEntry, LogEntryKind,
    TestQuality, TestRunStatus, REGEX_LOG_DESCRIPTION, REGEX_LOG_TASK_ID,
};
use crate::testgen::service::{GenerationRequest, TestGenEvent, TestGenService};
use testgen::TestGenContext;

/// Main RegexCrusade screen
pub struct RegexCrusadeScreen {
    /// Task info panel (left)
    task_panel: Entity<TaskPanel>,
    /// Test list panel (center)
    test_panel: Entity<TestPanel>,
    /// Iteration log (right) - shows streaming FM activity
    iteration_log: Entity<IterationLog>,
    /// Current session state
    session: CrusadeSession,
    /// TestGen service for background generation
    testgen_service: Arc<TestGenService>,
    /// Event receiver from TestGen
    event_receiver: Option<mpsc::UnboundedReceiver<TestGenEvent>>,
    /// Accumulated tests (we keep master list here to avoid losing tests)
    accumulated_tests: Vec<CrusadeTest>,
    /// Log entries for streaming display
    log_entries: Vec<LogEntry>,
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
            accumulated_tests: Vec::new(),
            log_entries: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Add a log entry and update the iteration log panel
    fn add_log(&mut self, kind: LogEntryKind, message: String, cx: &mut Context<Self>) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            kind,
            message,
        };
        self.log_entries.push(entry.clone());

        // Update iteration log panel
        self.iteration_log.update(cx, |log, cx| {
            log.add_log_entry(entry, cx);
        });
    }

    /// Start test generation
    fn start_generation(&mut self, cx: &mut Context<Self>) {
        if self.testgen_service.is_generating() {
            return;
        }

        // Clear current state
        self.accumulated_tests.clear();
        self.log_entries.clear();
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
        self.iteration_log.update(cx, |log, cx| {
            log.clear_logs(cx);
        });

        // Log start
        self.add_log(
            LogEntryKind::Info,
            format!("Starting test generation for task: {}", REGEX_LOG_TASK_ID),
            cx,
        );
        self.add_log(
            LogEntryKind::Prompt,
            format!("Task description:\n{}", REGEX_LOG_DESCRIPTION),
            cx,
        );

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
                    let cat_str = category
                        .map(|c| format!("{:?}", c))
                        .unwrap_or_else(|| "all".to_string());
                    let msg = format!("[{}] {} round {} - {}", phase, cat_str, round, status_msg);
                    self.add_log(LogEntryKind::Progress, msg, cx);
                }
                TestGenEvent::TestGenerated(test) => {
                    // Debug: check why tests might be stubs
                    let input_trimmed = test.input.trim();
                    let reasoning_trimmed = test.reasoning.trim();
                    let debug_info = format!(
                        "[in_len={}, in_trim={}, reason_len={}, reason_trim={}]",
                        test.input.len(),
                        input_trimmed.len(),
                        test.reasoning.len(),
                        reasoning_trimmed.len()
                    );

                    // Log the test
                    self.add_log(
                        LogEntryKind::TestGenerated,
                        format!(
                            "Test {}: {:?} {}\n  Input: \"{}\"\n  Expected: {:?}\n  Reasoning: {}",
                            test.id,
                            test.category,
                            debug_info,
                            truncate(&test.input, 60),
                            test.expected_output,
                            truncate(&test.reasoning, 80)
                        ),
                        cx,
                    );

                    // Convert and accumulate
                    let crusade_test = convert_test(&test);
                    self.accumulated_tests.push(crusade_test);
                    tests_updated = true;
                }
                TestGenEvent::Reflection(entry) => {
                    self.add_log(
                        LogEntryKind::Reflection,
                        format!(
                            "Reflection ({:?}): {}",
                            entry.action,
                            truncate(&entry.reflection_text, 100)
                        ),
                        cx,
                    );
                }
                TestGenEvent::Complete {
                    total_tests,
                    total_rounds,
                    duration_ms,
                } => {
                    self.add_log(
                        LogEntryKind::Complete,
                        format!(
                            "Generation complete: {} tests, {} rounds, {:.1}s",
                            total_tests,
                            total_rounds,
                            duration_ms as f64 / 1000.0
                        ),
                        cx,
                    );
                    self.session.status = CrusadeStatus::Idle;
                    should_clear_receiver = true;
                }
                TestGenEvent::Error(err) => {
                    self.add_log(LogEntryKind::Error, format!("Error: {}", err), cx);
                    self.session.status = CrusadeStatus::Failed;
                    should_clear_receiver = true;
                }
            }
        }

        if should_clear_receiver {
            self.event_receiver = None;
        }

        // Update UI with ALL accumulated tests
        if tests_updated {
            // Update counts from accumulated tests
            let stub_count = self
                .accumulated_tests
                .iter()
                .filter(|t| t.quality == TestQuality::Stub)
                .count() as u32;
            let real_count = self
                .accumulated_tests
                .iter()
                .filter(|t| t.quality == TestQuality::Real)
                .count() as u32;

            self.session.stub_count = stub_count;
            self.session.real_count = real_count;
            self.session.tests_total = self.accumulated_tests.len() as u32;

            // Clone the full list for the panel
            let all_tests = self.accumulated_tests.clone();
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
                    // Right: Streaming Log
                    .child(
                        div()
                            .w(px(380.0))
                            .h_full()
                            .child(self.iteration_log.clone()),
                    ),
            )
    }
}

/// Truncate a string for display
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
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
        _ => CrusadeCategory::Correctness,
    };

    // Tests with actual input and reasoning are REAL tests
    // Trim whitespace to handle edge cases
    let input_trimmed = test.input.trim();
    let reasoning_trimmed = test.reasoning.trim();

    let quality = if input_trimmed.is_empty() {
        TestQuality::Stub
    } else if input_trimmed.contains("TODO") || input_trimmed.contains("placeholder") {
        TestQuality::Suspicious
    } else if reasoning_trimmed.is_empty() {
        TestQuality::Suspicious
    } else {
        // Has real input and reasoning = REAL test
        TestQuality::Real
    };

    // Build actual test code showing the assertion
    let code = format!(
        "# Test: {}\n# Reasoning: {}\n\ninput = \"{}\"\nexpected = {:?}\nresult = regex.findall(pattern, input)\nassert result == expected",
        test.id,
        test.reasoning,
        test.input.replace("\"", "\\\""),
        test.expected_output
    );

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
