//! TestGen Visualizer - Main TestGen view

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use testgen::TestGenContext;
use theme::{bg, border, status, text, FONT_FAMILY};
use tokio::sync::mpsc;

use super::category_progress::{CategoryProgress, OnCategorySelectCallback, TestCategory};
use super::service::{load_latest_generation, GenerationRequest, TestGenEvent, TestGenService};
use super::test_detail::{TestDetail, TestInfo};
use super::test_list::{OnSelectCallback, TestCase, TestList, TestStatus};

use crate::services::TaskLoader;
use crate::tbcc::types::TBTask;

/// TestGen session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestGenStatus {
    #[default]
    Idle,
    Generating,
    Completed,
    Failed,
}

impl TestGenStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Generating => "Generating",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

/// Generation status with detailed progress tracking
#[derive(Debug, Clone, Default)]
pub enum GenerationStatus {
    #[default]
    Idle,
    Generating {
        iteration: u32,
        max_iterations: u32,
        tests_so_far: u32,
    },
    Complete {
        total_tests: u32,
        duration_ms: u64,
    },
    Failed {
        error: String,
    },
}

impl GenerationStatus {
    pub fn is_generating(&self) -> bool {
        matches!(self, Self::Generating { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Generating { .. } => "Generating...",
            Self::Complete { .. } => "Complete",
            Self::Failed { .. } => "Failed",
        }
    }
}

/// TestGen session
#[derive(Debug, Clone)]
pub struct TestGenSession {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub status: TestGenStatus,
    pub iteration: u32,
    pub max_iterations: u32,
    pub comprehensiveness: f32,
    pub target_comprehensiveness: f32,
}

/// Main TestGen Visualizer view
pub struct TestGenVisualizer {
    pub session: Option<TestGenSession>,
    category_progress: Entity<CategoryProgress>,
    /// Test list component (public for testing)
    pub test_list: Entity<TestList>,
    /// Test detail component (public for testing)
    pub test_detail: Entity<TestDetail>,
    pub selected_test_id: Option<String>,
    focus_handle: FocusHandle,

    // Task selection state
    pub available_tasks: Vec<TBTask>,
    pub selected_task_idx: Option<usize>,
    pub generation_status: GenerationStatus,

    // Service integration
    service: Arc<TestGenService>,
    event_receiver: Option<mpsc::UnboundedReceiver<TestGenEvent>>,
    pub generated_tests: Vec<testgen::GeneratedTest>,
}

impl TestGenVisualizer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Load available tasks from TaskLoader
        let task_loader = TaskLoader::new();
        let available_tasks = task_loader.load_all_tasks();

        // Start with no session (user must select task and generate)
        let session: Option<TestGenSession> = None;

        // Initialize components with empty state
        let category_progress = cx.new(|cx| CategoryProgress::new(cx));

        let test_list = cx.new(|cx| TestList::new(cx));

        // Clear sample test from detail view - start empty
        let test_detail = cx.new(|cx| {
            let mut detail = TestDetail::new(cx);
            detail.set_test(None);
            detail
        });

        // Create the service
        let service = Arc::new(TestGenService::new());

        // Wire up callbacks after creating entities
        // Test list -> Test detail callback
        let test_detail_entity = test_detail.clone();
        let on_test_select: OnSelectCallback = Arc::new(move |test: &TestCase, _window, cx| {
            test_detail_entity.update(cx, |detail, _cx| {
                detail.set_test(Some(TestInfo {
                    id: test.id.clone(),
                    name: test.name.clone(),
                    category: test.category,
                    status: test.status,
                    description: test.description.clone(),
                    code: test.code.clone(),
                    confidence: test.confidence,
                    reasoning: None,
                }));
            });
        });
        test_list.update(cx, |list, _cx| {
            list.set_on_select(on_test_select);
        });

        // Category progress -> Test list filter callback
        let test_list_entity = test_list.clone();
        let on_category_select: OnCategorySelectCallback =
            Arc::new(move |category: Option<TestCategory>, _window, cx| {
                test_list_entity.update(cx, |list, _cx| {
                    list.filter_by_category(category);
                });
            });
        category_progress.update(cx, |progress, _cx| {
            progress.set_on_category_select(on_category_select);
        });

        Self {
            session,
            category_progress,
            test_list,
            test_detail,
            selected_test_id: None,
            focus_handle: cx.focus_handle(),
            available_tasks,
            selected_task_idx: None,
            generation_status: GenerationStatus::Idle,
            service,
            event_receiver: None,
            generated_tests: Vec::new(),
        }
    }

    /// Get the currently selected task, if any
    pub fn selected_task(&self) -> Option<&TBTask> {
        self.selected_task_idx
            .and_then(|idx| self.available_tasks.get(idx))
    }

    /// Select a task by index
    pub fn select_task(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.available_tasks.len() {
            self.selected_task_idx = Some(idx);
            self.generation_status = GenerationStatus::Idle;
            // Clear previous session when new task selected
            self.session = None;

            // Clear current tests
            self.generated_tests.clear();
            self.test_list.update(cx, |list, _cx| {
                list.clear_tests();
            });
            self.test_detail.update(cx, |detail, _cx| {
                detail.set_test(None);
            });

            // Try to load previous generation
            let task = &self.available_tasks[idx];
            if let Some(saved) = load_latest_generation(&task.id) {
                // Update status to show we have previous results
                self.generation_status = GenerationStatus::Complete {
                    total_tests: saved.total_tests as u32,
                    duration_ms: 0, // Unknown for loaded results
                };

                // Populate test list with saved tests
                for test in &saved.tests {
                    let test_case = convert_generated_test(test);
                    self.test_list.update(cx, |list, _cx| {
                        list.add_test(test_case);
                    });
                }
                self.generated_tests = saved.tests;
            }

            cx.notify();
        }
    }

    /// Select next task in list
    pub fn select_next_task(&mut self, cx: &mut Context<Self>) {
        if self.available_tasks.is_empty() {
            return;
        }
        let next_idx = match self.selected_task_idx {
            Some(idx) => (idx + 1) % self.available_tasks.len(),
            None => 0,
        };
        self.select_task(next_idx, cx);
    }

    /// Select previous task in list
    pub fn select_prev_task(&mut self, cx: &mut Context<Self>) {
        if self.available_tasks.is_empty() {
            return;
        }
        let prev_idx = match self.selected_task_idx {
            Some(idx) => {
                if idx == 0 {
                    self.available_tasks.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.available_tasks.len() - 1,
        };
        self.select_task(prev_idx, cx);
    }

    /// Check if generation can start (task selected and not already generating)
    pub fn can_generate(&self) -> bool {
        self.selected_task_idx.is_some() && !self.generation_status.is_generating()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_generate = self.can_generate();
        let is_generating = self.generation_status.is_generating();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            // Left side: Title + Task Selector
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("TestGen"),
                    )
                    // Task selector
                    .child(self.render_task_selector(cx)),
            )
            // Right side: Status + Generate Button
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    // Status indicator
                    .child(self.render_generation_status())
                    // Generate button
                    .child(
                        div()
                            .px(px(16.0))
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
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .font_weight(FontWeight::MEDIUM)
                            .id("generate-tests-btn")
                            .when(can_generate && !is_generating, |el| {
                                el.on_click(cx.listener(|this, _evt, _window, cx| {
                                    this.start_generation(cx);
                                }))
                            })
                            .child(if is_generating {
                                "Generating..."
                            } else {
                                "Generate Tests"
                            }),
                    ),
            )
    }

    fn render_task_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_task = self.selected_task();
        let task_count = self.available_tasks.len();

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            // Prev button
            .child(
                div()
                    .id("task-prev-btn")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .cursor(CursorStyle::PointingHand)
                    .bg(bg::ELEVATED)
                    .text_color(text::MUTED)
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .on_click(cx.listener(|this, _evt, _window, cx| {
                        this.select_prev_task(cx);
                    }))
                    .child("<"),
            )
            // Task name display
            .child(
                div()
                    .min_w(px(200.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(bg::ELEVATED)
                    .border_1()
                    .border_color(border::SUBTLE)
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if selected_task.is_some() {
                                text::PRIMARY
                            } else {
                                text::MUTED
                            })
                            .child(
                                selected_task
                                    .map(|t| t.name.clone())
                                    .unwrap_or_else(|| "Select a task...".to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!(
                                "{}/{}",
                                self.selected_task_idx.map(|i| i + 1).unwrap_or(0),
                                task_count
                            )),
                    ),
            )
            // Next button
            .child(
                div()
                    .id("task-next-btn")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .cursor(CursorStyle::PointingHand)
                    .bg(bg::ELEVATED)
                    .text_color(text::MUTED)
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .on_click(cx.listener(|this, _evt, _window, cx| {
                        this.select_next_task(cx);
                    }))
                    .child(">"),
            )
    }

    fn render_generation_status(&self) -> impl IntoElement {
        let (bg_color, text_color, label) = match &self.generation_status {
            GenerationStatus::Idle => (bg::ELEVATED, text::MUTED, "Ready".to_string()),
            GenerationStatus::Generating {
                iteration,
                max_iterations,
                tests_so_far,
            } => (
                status::INFO_BG,
                status::RUNNING,
                format!(
                    "Iteration {}/{} ({} tests)",
                    iteration, max_iterations, tests_so_far
                ),
            ),
            GenerationStatus::Complete {
                total_tests,
                duration_ms,
            } => (
                status::SUCCESS_BG,
                status::SUCCESS,
                format!(
                    "{} tests in {:.1}s",
                    total_tests, *duration_ms as f64 / 1000.0
                ),
            ),
            GenerationStatus::Failed { error } => (
                status::ERROR_BG,
                status::ERROR,
                format!("Failed: {}", error.chars().take(30).collect::<String>()),
            ),
        };

        div()
            .px(px(10.0))
            .py(px(4.0))
            .bg(bg_color)
            .text_color(text_color)
            .text_size(px(11.0))
            .font_family(FONT_FAMILY)
            .font_weight(FontWeight::MEDIUM)
            .rounded(px(4.0))
            .child(label)
    }

    /// Start test generation (placeholder - will be wired in Phase 2)
    /// Start test generation using the service
    fn start_generation(&mut self, cx: &mut Context<Self>) {
        if let Some(task) = self.selected_task().cloned() {
            // Clear previous results
            self.generated_tests.clear();

            // Update status
            self.generation_status = GenerationStatus::Generating {
                iteration: 1,
                max_iterations: 8,
                tests_so_far: 0,
            };

            // Create generation request
            let request = GenerationRequest {
                task_id: task.id.clone(),
                task_description: task.description.clone(),
                context: TestGenContext::Benchmark,
            };

            // Start generation and get event receiver
            let receiver = self.service.start_generation(request);
            self.event_receiver = Some(receiver);

            // Schedule polling
            cx.spawn(async move |this, cx| {
                loop {
                    // Small delay between polls
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;

                    // Poll for events
                    let should_continue = this
                        .update(cx, |this, cx| this.poll_events(cx))
                        .unwrap_or(false);

                    if !should_continue {
                        break;
                    }
                }
            })
            .detach();

            cx.notify();
        }
    }

    /// Poll for events from the generation service
    fn poll_events(&mut self, cx: &mut Context<Self>) -> bool {
        // Take the receiver temporarily
        let mut receiver = match self.event_receiver.take() {
            Some(r) => r,
            None => return false,
        };

        let mut should_continue = true;
        let mut events = Vec::new();

        // Collect all available events
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }

        // Process events
        for event in events {
            match event {
                TestGenEvent::Progress { round, .. } => {
                    // Update iteration
                    if let GenerationStatus::Generating {
                        ref mut iteration, ..
                    } = self.generation_status
                    {
                        *iteration = round;
                    }
                    cx.notify();
                }
                TestGenEvent::TestGenerated(test) => {
                    // Add test to list
                    self.generated_tests.push(test.clone());

                    // Update test count in status
                    if let GenerationStatus::Generating {
                        ref mut tests_so_far,
                        ..
                    } = self.generation_status
                    {
                        *tests_so_far = self.generated_tests.len() as u32;
                    }

                    // Update test list UI
                    self.test_list.update(cx, |list, _cx| {
                        let test_case = convert_generated_test(&test);
                        list.add_test(test_case);
                    });

                    cx.notify();
                }
                TestGenEvent::Complete {
                    total_tests,
                    duration_ms,
                    ..
                } => {
                    self.generation_status = GenerationStatus::Complete {
                        total_tests,
                        duration_ms,
                    };
                    should_continue = false;
                    cx.notify();
                }
                TestGenEvent::Error(error) => {
                    self.generation_status = GenerationStatus::Failed { error };
                    should_continue = false;
                    cx.notify();
                }
                TestGenEvent::Reflection(_) => {
                    // Could update UI with reflection info
                }
            }
        }

        // Put receiver back if we should continue
        if should_continue {
            self.event_receiver = Some(receiver);
        }

        should_continue
    }

    fn render_comprehensiveness(&self) -> impl IntoElement {
        let session = self.session.as_ref();

        div()
            .flex()
            .items_center()
            .gap(px(16.0))
            .px(px(20.0))
            .py(px(12.0))
            .bg(bg::SURFACE)
            .border_b_1()
            .border_color(border::DEFAULT)
            .when_some(session, |el, s| {
                let progress = s.comprehensiveness / s.target_comprehensiveness;
                let progress_width = (progress.min(1.0) * 200.0).max(4.0);
                let is_met = s.comprehensiveness >= s.target_comprehensiveness;

                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Comprehensiveness Score"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .text_size(px(24.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(if is_met {
                                            status::SUCCESS
                                        } else {
                                            text::PRIMARY
                                        })
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("{:.0}%", s.comprehensiveness * 100.0)),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(format!(
                                            "/ {:.0}% target",
                                            s.target_comprehensiveness * 100.0
                                        )),
                                ),
                        ),
                )
                .child(
                    div()
                        .w(px(200.0))
                        .h(px(8.0))
                        .bg(bg::ELEVATED)
                        .rounded(px(4.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .w(px(progress_width))
                                .h_full()
                                .bg(if is_met { status::SUCCESS } else { status::INFO })
                                .rounded(px(4.0)),
                        ),
                )
            })
    }
}

impl Focusable for TestGenVisualizer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestGenVisualizer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Header with task selector and generate button
            .child(self.render_header(cx))
            // Comprehensiveness bar (only when session active)
            .child(self.render_comprehensiveness())
            // Main content
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Left: Category Progress
                    .child(
                        div()
                            .w(px(280.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(border::DEFAULT)
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(10.0))
                                    .border_b_1()
                                    .border_color(border::SUBTLE)
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Categories"),
                                    ),
                            )
                            .child(div().flex_1().child(self.category_progress.clone())),
                    )
                    // Center: Test List
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(border::DEFAULT)
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
                                            .child("Generated Tests"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::DISABLED)
                                            .child("8 tests"),
                                    ),
                            )
                            .child(div().flex_1().child(self.test_list.clone())),
                    )
                    // Right: Test Detail
                    .child(
                        div()
                            .w(px(400.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(10.0))
                                    .border_b_1()
                                    .border_color(border::SUBTLE)
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Test Details"),
                                    ),
                            )
                            .child(div().flex_1().child(self.test_detail.clone())),
                    ),
            )
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a testgen::GeneratedTest to our UI TestCase
fn convert_generated_test(test: &testgen::GeneratedTest) -> TestCase {
    // Map testgen category to UI category
    let category = match test.category {
        testgen::TestCategory::AntiCheat => TestCategory::AntiCheat,
        testgen::TestCategory::Existence => TestCategory::Existence,
        testgen::TestCategory::Correctness => TestCategory::Correctness,
        testgen::TestCategory::Boundary => TestCategory::Boundary,
        testgen::TestCategory::Integration => TestCategory::Integration,
        // Map additional testgen categories to nearest UI equivalent
        testgen::TestCategory::Format => TestCategory::Correctness,
        testgen::TestCategory::HappyPath => TestCategory::Correctness,
        testgen::TestCategory::EdgeCase => TestCategory::Boundary,
        testgen::TestCategory::InvalidInput => TestCategory::Boundary,
    };

    TestCase {
        id: test.id.clone(),
        name: test.id.clone(),
        category,
        status: TestStatus::Generated,
        description: test.reasoning.clone(),
        code: format!(
            "# Input: {}\n# Expected: {}\n\n{}",
            test.input,
            test.expected_output.as_deref().unwrap_or("N/A"),
            test.reasoning
        ),
        confidence: test.confidence as f32,
    }
}
