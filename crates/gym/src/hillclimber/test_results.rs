//! Test results display (X/Y passed, failed test names)

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

/// Test outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestOutcome {
    #[default]
    Pending,
    Passed,
    Failed,
    Skipped,
}

impl TestOutcome {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "◦",
            Self::Passed => "✓",
            Self::Failed => "✗",
            Self::Skipped => "−",
        }
    }
}

/// A test result
#[derive(Debug, Clone)]
pub struct TestResult {
    pub id: String,
    pub name: String,
    pub outcome: TestOutcome,
    pub duration_ms: Option<u32>,
}

/// Test results display component
pub struct TestResults {
    results: Vec<TestResult>,
    filter: Option<TestOutcome>,
    focus_handle: FocusHandle,
}

impl TestResults {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            results: Vec::new(),
            filter: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_results(&mut self, results: Vec<TestResult>) {
        self.results = results;
    }

    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }

    pub fn update_result(&mut self, id: &str, outcome: TestOutcome, duration_ms: Option<u32>) {
        if let Some(result) = self.results.iter_mut().find(|r| r.id == id) {
            result.outcome = outcome;
            result.duration_ms = duration_ms;
        }
    }

    fn render_result(&self, result: &TestResult, is_last: bool) -> impl IntoElement {
        let (icon_color, bg_color) = match result.outcome {
            TestOutcome::Pending => (text::MUTED, bg::ROW),
            TestOutcome::Passed => (status::SUCCESS, bg::ROW),
            TestOutcome::Failed => (status::ERROR, status::ERROR_BG.opacity(0.3)),
            TestOutcome::Skipped => (text::DISABLED, bg::ROW),
        };

        let name = result.name.clone();
        let icon = result.outcome.icon();
        let duration = result.duration_ms.map(|ms| format!("{}ms", ms)).unwrap_or_else(|| "−".to_string());

        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(6.0))
            .bg(bg_color)
            .when(!is_last, |el| {
                el.border_b_1().border_color(border::SUBTLE)
            })
            .hover(|el| el.bg(bg::HOVER))
            // Status icon
            .child(
                div()
                    .w(px(16.0))
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(icon_color)
                    .child(icon)
            )
            // Test name
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name)
            )
            // Duration
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(duration)
            )
    }

    fn render_summary(&self) -> impl IntoElement {
        let passed = self.results.iter().filter(|r| r.outcome == TestOutcome::Passed).count();
        let failed = self.results.iter().filter(|r| r.outcome == TestOutcome::Failed).count();
        let total = self.results.len();

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(bg::ELEVATED)
            .border_b_1()
            .border_color(border::DEFAULT)
            // Passed count
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(status::SUCCESS)
                            .child("✓")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(format!("{} passed", passed))
                    )
            )
            // Failed count
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(status::ERROR)
                            .child("✗")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(format!("{} failed", failed))
                    )
            )
            // Total
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} total", total))
                    )
            )
    }
}

impl Focusable for TestResults {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestResults {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let result_count = self.results.len();

        // Filter results if filter is set
        let filtered_results: Vec<_> = self.results.iter()
            .filter(|r| self.filter.map_or(true, |f| r.outcome == f))
            .collect();

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            // Summary bar
            .child(self.render_summary())
            // Results list
            .child(
                div()
                    .id("test-results-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(filtered_results.is_empty(), |el| {
                        el.flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("No test results")
                            )
                    })
                    .when(!filtered_results.is_empty(), |el| {
                        el.children(
                            filtered_results.iter().enumerate().map(|(idx, result)| {
                                self.render_result(result, idx == result_count - 1)
                            })
                        )
                    })
            )
    }
}
