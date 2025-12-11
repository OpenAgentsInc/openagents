//! Test Panel - List of tests with stub detection

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, category, status, text, FONT_FAMILY};

use super::types::{CrusadeCategory, CrusadeTest, TestQuality, TestRunStatus};

/// Test panel component
pub struct TestPanel {
    tests: Vec<CrusadeTest>,
    selected_id: Option<String>,
    focus_handle: FocusHandle,
}

impl TestPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tests: Vec::new(),
            selected_id: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_tests(&mut self, tests: Vec<CrusadeTest>, cx: &mut Context<Self>) {
        self.tests = tests;
        cx.notify();
    }

    pub fn get_stub_count(&self) -> u32 {
        self.tests
            .iter()
            .filter(|t| t.quality == TestQuality::Stub)
            .count() as u32
    }

    pub fn get_real_count(&self) -> u32 {
        self.tests
            .iter()
            .filter(|t| t.quality == TestQuality::Real)
            .count() as u32
    }

    fn get_passed_count(&self) -> u32 {
        self.tests
            .iter()
            .filter(|t| t.status == TestRunStatus::Passed)
            .count() as u32
    }

    fn render_header(&self) -> impl IntoElement {
        let total = self.tests.len();
        let passed = self.get_passed_count();
        let stub_count = self.get_stub_count();
        let real_count = self.get_real_count();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::ELEVATED)
            // Left: Title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
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
                            .child(format!("{} total", total)),
                    ),
            )
            // Right: Quality summary
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Real count
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(status::SUCCESS_BG)
                            .rounded(px(3.0))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::SUCCESS)
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{} real", real_count)),
                    )
                    // Stub count (warning if > 0)
                    .when(stub_count > 0, |el| {
                        el.child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(status::ERROR_BG)
                                .rounded(px(3.0))
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(status::ERROR)
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{} stubs", stub_count)),
                        )
                    })
                    // Pass rate
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::SUCCESS)
                            .child(format!("{}/{} passing", passed, total)),
                    ),
            )
    }

    fn render_test_row(&self, test: &CrusadeTest, idx: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let is_selected = self.selected_id.as_deref() == Some(&test.id);
        let test_id = test.id.clone();

        // Quality badge colors
        let (quality_bg, quality_text) = match test.quality {
            TestQuality::Real => (status::SUCCESS_BG, status::SUCCESS),
            TestQuality::Stub => (status::ERROR_BG, status::ERROR),
            TestQuality::Suspicious => (status::WARNING_BG, status::WARNING),
            TestQuality::Unknown => (bg::ELEVATED, text::MUTED),
        };

        // Status colors
        let status_color = match test.status {
            TestRunStatus::NotRun => text::MUTED,
            TestRunStatus::Running => status::RUNNING,
            TestRunStatus::Passed => status::SUCCESS,
            TestRunStatus::Failed => status::ERROR,
            TestRunStatus::Error => status::ERROR,
        };

        // Category colors
        let (cat_bg, cat_text) = match test.category {
            CrusadeCategory::AntiCheat => (category::ANTI_CHEAT_BG, category::ANTI_CHEAT_TEXT),
            CrusadeCategory::Existence => (category::EXISTENCE_BG, category::EXISTENCE_TEXT),
            CrusadeCategory::Correctness => (category::CORRECTNESS_BG, category::CORRECTNESS_TEXT),
            CrusadeCategory::Boundary => (category::BOUNDARY_BG, category::BOUNDARY_TEXT),
            CrusadeCategory::Integration => (category::INTEGRATION_BG, category::INTEGRATION_TEXT),
        };

        let input_preview: String = test.input.chars().take(40).collect();

        div()
            .id(ElementId::Name(format!("test-row-{}", idx).into()))
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(if is_selected { bg::SELECTED } else { bg::ROW })
            .border_b_1()
            .border_color(border::SUBTLE)
            .when(is_selected, |el| {
                el.border_l_2().border_color(border::SELECTED)
            })
            .hover(|el| el.bg(if is_selected { bg::SELECTED } else { bg::HOVER }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    this.selected_id = Some(test_id.clone());
                    cx.notify();
                }),
            )
            // Status icon
            .child(
                div()
                    .w(px(16.0))
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status_color)
                    .child(test.status.icon()),
            )
            // Quality badge
            .child(
                div()
                    .w(px(20.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .bg(quality_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(quality_text)
                            .font_weight(FontWeight::BOLD)
                            .child(test.quality.icon()),
                    ),
            )
            // Category badge
            .child(
                div()
                    .w(px(24.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .bg(cat_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(7.0))
                            .font_family(FONT_FAMILY)
                            .text_color(cat_text)
                            .font_weight(FontWeight::BOLD)
                            .child(test.category.icon()),
                    ),
            )
            // Test ID + input preview
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_selected {
                                text::BRIGHT
                            } else {
                                text::PRIMARY
                            })
                            .text_ellipsis()
                            .child(test.id.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .text_ellipsis()
                            .child(format!("\"{}\"", input_preview)),
                    ),
            )
            // Confidence
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(format!("{:.0}%", test.confidence * 100.0)),
            )
    }
}

impl Focusable for TestPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Pre-render all rows using a for loop (closures can't capture cx)
        let mut rows = Vec::with_capacity(self.tests.len());
        for (idx, test) in self.tests.iter().enumerate() {
            rows.push(self.render_test_row(test, idx, cx));
        }

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            .child(self.render_header())
            .child(
                div()
                    .id("test-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(rows.is_empty(), |el| {
                        el.flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("No tests generated yet"),
                            )
                    })
                    .when(!rows.is_empty(), |el| el.children(rows)),
            )
    }
}
