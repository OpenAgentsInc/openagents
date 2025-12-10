//! TestGen Visualizer - Main TestGen view

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::category_progress::{CategoryProgress, TestCategory, CategoryStats};
use super::test_list::{TestList, TestCase, TestStatus};
use super::test_detail::TestDetail;

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
    test_list: Entity<TestList>,
    test_detail: Entity<TestDetail>,
    pub selected_test_id: Option<String>,
    focus_handle: FocusHandle,
}

impl TestGenVisualizer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Sample session
        let session = Some(TestGenSession {
            id: "tg-001".to_string(),
            task_id: "regex-log".to_string(),
            task_name: "Regex Log Parser".to_string(),
            status: TestGenStatus::Generating,
            iteration: 3,
            max_iterations: 5,
            comprehensiveness: 0.72,
            target_comprehensiveness: 0.85,
        });

        // Initialize components with sample data
        let category_progress = cx.new(|cx| {
            let mut progress = CategoryProgress::new(cx);
            progress.set_stats(Self::create_sample_stats());
            progress
        });

        let test_list = cx.new(|cx| {
            let mut list = TestList::new(cx);
            list.set_tests(Self::create_sample_tests());
            list
        });

        let test_detail = cx.new(|cx| TestDetail::new(cx));

        Self {
            session,
            category_progress,
            test_list,
            test_detail,
            selected_test_id: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn create_sample_stats() -> Vec<CategoryStats> {
        vec![
            CategoryStats {
                category: TestCategory::AntiCheat,
                generated: 4,
                target: 5,
                passed: 3,
            },
            CategoryStats {
                category: TestCategory::Existence,
                generated: 3,
                target: 4,
                passed: 3,
            },
            CategoryStats {
                category: TestCategory::Correctness,
                generated: 5,
                target: 6,
                passed: 4,
            },
            CategoryStats {
                category: TestCategory::Boundary,
                generated: 2,
                target: 4,
                passed: 1,
            },
            CategoryStats {
                category: TestCategory::Integration,
                generated: 1,
                target: 3,
                passed: 1,
            },
        ]
    }

    fn create_sample_tests() -> Vec<TestCase> {
        vec![
            TestCase {
                id: "test-ac-1".to_string(),
                name: "anti_cheat_hardcoded_check".to_string(),
                category: TestCategory::AntiCheat,
                status: TestStatus::Passed,
                description: "Ensures solution doesn't hardcode specific test values".to_string(),
                code: r#"def test_no_hardcoded():
    """Anti-cheat: Verify no hardcoded values"""
    result = parse_log("random_input_xyz")
    assert result != KNOWN_HARDCODED"#.to_string(),
                confidence: 0.95,
            },
            TestCase {
                id: "test-ac-2".to_string(),
                name: "anti_cheat_generalization".to_string(),
                category: TestCategory::AntiCheat,
                status: TestStatus::Passed,
                description: "Tests generalization across input variations".to_string(),
                code: r#"def test_generalization():
    """Anti-cheat: Verify generalization"""
    inputs = generate_variations()
    for inp in inputs:
        assert parse_log(inp) is not None"#.to_string(),
                confidence: 0.88,
            },
            TestCase {
                id: "test-ex-1".to_string(),
                name: "existence_basic_parse".to_string(),
                category: TestCategory::Existence,
                status: TestStatus::Passed,
                description: "Basic parsing capability test".to_string(),
                code: r#"def test_basic_parse():
    """Existence: Basic parsing works"""
    log = "192.168.1.1 - - [10/Dec/2024:14:30:00]"
    result = parse_log(log)
    assert result is not None"#.to_string(),
                confidence: 0.99,
            },
            TestCase {
                id: "test-co-1".to_string(),
                name: "correctness_ip_extraction".to_string(),
                category: TestCategory::Correctness,
                status: TestStatus::Passed,
                description: "IP address extraction correctness".to_string(),
                code: r#"def test_ip_extraction():
    """Correctness: IP extracted correctly"""
    log = "192.168.1.1 - - [timestamp]"
    result = parse_log(log)
    assert result["ip"] == "192.168.1.1""#.to_string(),
                confidence: 0.92,
            },
            TestCase {
                id: "test-co-2".to_string(),
                name: "correctness_timestamp".to_string(),
                category: TestCategory::Correctness,
                status: TestStatus::Failed,
                description: "Timestamp parsing correctness".to_string(),
                code: r#"def test_timestamp():
    """Correctness: Timestamp parsed correctly"""
    log = "... [10/Dec/2024:14:30:00 +0000]"
    result = parse_log(log)
    assert result["timestamp"] == expected"#.to_string(),
                confidence: 0.78,
            },
            TestCase {
                id: "test-bo-1".to_string(),
                name: "boundary_empty_input".to_string(),
                category: TestCategory::Boundary,
                status: TestStatus::Passed,
                description: "Empty input handling".to_string(),
                code: r#"def test_empty_input():
    """Boundary: Empty input handled"""
    result = parse_log("")
    assert result is None or result == {}"#.to_string(),
                confidence: 0.85,
            },
            TestCase {
                id: "test-bo-2".to_string(),
                name: "boundary_malformed".to_string(),
                category: TestCategory::Boundary,
                status: TestStatus::Running,
                description: "Malformed input handling".to_string(),
                code: r#"def test_malformed():
    """Boundary: Malformed input handled"""
    result = parse_log("not a valid log")
    # Should not crash"#.to_string(),
                confidence: 0.72,
            },
            TestCase {
                id: "test-in-1".to_string(),
                name: "integration_multiline".to_string(),
                category: TestCategory::Integration,
                status: TestStatus::Generated,
                description: "Multi-line log processing".to_string(),
                code: r#"def test_multiline():
    """Integration: Multi-line logs"""
    logs = """line1
    line2
    line3"""
    results = parse_logs(logs)
    assert len(results) == 3"#.to_string(),
                confidence: 0.65,
            },
        ]
    }

    fn render_header(&self) -> impl IntoElement {
        let session = self.session.as_ref();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
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
                            .child("TestGen Visualizer")
                    )
                    .when_some(session, |el, s| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("Task:")
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::PRIMARY)
                                        .child(s.task_name.clone())
                                )
                        )
                    })
            )
            .when_some(session, |el, s| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(self.render_status_badge(s.status))
                        .child(self.render_iteration_indicator(s))
                )
            })
    }

    fn render_status_badge(&self, status: TestGenStatus) -> impl IntoElement {
        let (bg_color, text_color, label) = match status {
            TestGenStatus::Idle => (bg::ELEVATED, text::MUTED, "Idle"),
            TestGenStatus::Generating => (status::INFO_BG, status::RUNNING, "Generating"),
            TestGenStatus::Completed => (status::SUCCESS_BG, status::SUCCESS, "Completed"),
            TestGenStatus::Failed => (status::ERROR_BG, status::ERROR, "Failed"),
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

    fn render_iteration_indicator(&self, session: &TestGenSession) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(format!("Iteration {}/{}", session.iteration, session.max_iterations))
            )
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

                el
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Comprehensiveness Score")
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
                                            .text_color(if is_met { status::SUCCESS } else { text::PRIMARY })
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("{:.0}%", s.comprehensiveness * 100.0))
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child(format!("/ {:.0}% target", s.target_comprehensiveness * 100.0))
                                    )
                            )
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
                                    .rounded(px(4.0))
                            )
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Header
            .child(self.render_header())
            // Comprehensiveness bar
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
                                            .child("Categories")
                                    )
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.category_progress.clone())
                            )
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
                                            .child("Generated Tests")
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::DISABLED)
                                            .child("8 tests")
                                    )
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.test_list.clone())
                            )
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
                                            .child("Test Details")
                                    )
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.test_detail.clone())
                            )
                    )
            )
    }
}
