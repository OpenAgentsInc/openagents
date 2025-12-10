//! Scrollable test case list

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::category_progress::TestCategory;

/// Test status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestStatus {
    #[default]
    Generated,
    Running,
    Passed,
    Failed,
    Skipped,
}

impl TestStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Generated => "◦",
            Self::Running => "◎",
            Self::Passed => "✓",
            Self::Failed => "✗",
            Self::Skipped => "−",
        }
    }
}

/// A test case
#[derive(Debug, Clone)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub category: TestCategory,
    pub status: TestStatus,
    pub description: String,
    pub code: String,
    pub confidence: f32,
}

/// Test list component
pub struct TestList {
    tests: Vec<TestCase>,
    selected_id: Option<String>,
    filter_category: Option<TestCategory>,
    focus_handle: FocusHandle,
}

impl TestList {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tests: Vec::new(),
            selected_id: None,
            filter_category: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_tests(&mut self, tests: Vec<TestCase>) {
        self.tests = tests;
    }

    pub fn select(&mut self, id: String) {
        self.selected_id = Some(id);
    }

    pub fn filter_by_category(&mut self, category: Option<TestCategory>) {
        self.filter_category = category;
    }

    fn render_test_row(&self, test: &TestCase) -> impl IntoElement {
        let is_selected = self.selected_id.as_deref() == Some(&test.id);

        let (status_color, status_bg) = match test.status {
            TestStatus::Generated => (text::MUTED, bg::ROW),
            TestStatus::Running => (status::RUNNING, bg::ROW),
            TestStatus::Passed => (status::SUCCESS, bg::ROW),
            TestStatus::Failed => (status::ERROR, status::ERROR_BG.opacity(0.3)),
            TestStatus::Skipped => (text::DISABLED, bg::ROW),
        };

        let name = test.name.clone();
        let category_icon = test.category.icon().to_string();
        let status_icon = test.status.icon();
        let confidence = format!("{:.0}%", test.confidence * 100.0);

        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(if is_selected { bg::SELECTED } else { status_bg })
            .border_b_1()
            .border_color(border::SUBTLE)
            .when(is_selected, |el| {
                el.border_l_2().border_color(border::SELECTED)
            })
            .hover(|el| el.bg(if is_selected { bg::SELECTED } else { bg::HOVER }))
            .cursor_pointer()
            // Status icon
            .child(
                div()
                    .w(px(16.0))
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status_color)
                    .child(status_icon)
            )
            // Category badge
            .child(
                div()
                    .w(px(24.0))
                    .h(px(18.0))
                    .rounded(px(2.0))
                    .bg(bg::ELEVATED)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .font_weight(FontWeight::BOLD)
                            .child(category_icon)
                    )
            )
            // Test name
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name)
            )
            // Confidence
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(confidence)
            )
    }
}

impl Focusable for TestList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestList {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Filter tests
        let filtered: Vec<_> = self.tests.iter()
            .filter(|t| self.filter_category.map_or(true, |c| t.category == c))
            .collect();

        div()
            .id("test-list-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .bg(bg::SURFACE)
            .when(filtered.is_empty(), |el| {
                el.flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("No tests generated yet")
                    )
            })
            .when(!filtered.is_empty(), |el| {
                el.children(filtered.iter().map(|test| {
                    self.render_test_row(test)
                }))
            })
    }
}
