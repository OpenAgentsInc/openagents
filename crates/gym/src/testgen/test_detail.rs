//! Test code viewer with details

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::category_progress::TestCategory;
use super::test_list::TestStatus;

/// Test detail information
#[derive(Debug, Clone)]
pub struct TestInfo {
    pub id: String,
    pub name: String,
    pub category: TestCategory,
    pub status: TestStatus,
    pub description: String,
    pub code: String,
    pub confidence: f32,
    pub reasoning: Option<String>,
}

/// Test detail viewer component
pub struct TestDetail {
    test: Option<TestInfo>,
    focus_handle: FocusHandle,
}

impl TestDetail {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Sample test for development
        let test = Some(TestInfo {
            id: "test-ac-1".to_string(),
            name: "anti_cheat_hardcoded_check".to_string(),
            category: TestCategory::AntiCheat,
            status: TestStatus::Passed,
            description: "Ensures the solution doesn't hardcode specific test values. Validates that the implementation uses actual parsing logic.".to_string(),
            code: r#"def test_no_hardcoded():
    """Anti-cheat: Verify no hardcoded values.

    This test checks that the solution doesn't
    simply return hardcoded values for known inputs.
    """
    # Generate random input
    random_input = generate_random_log()
    result = parse_log(random_input)

    # Should not match any hardcoded values
    assert result != KNOWN_HARDCODED_VALUES
    assert result is not None

    # Verify structure is correct
    assert "timestamp" in result
    assert "level" in result"#.to_string(),
            confidence: 0.95,
            reasoning: Some("Generated to detect hardcoded solutions by testing with random inputs that wouldn't be in any static lookup table.".to_string()),
        });

        Self {
            test,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_test(&mut self, test: Option<TestInfo>) {
        self.test = test;
    }

    fn render_header(&self, test: &TestInfo) -> impl IntoElement {
        let (status_color, status_bg, status_text) = match test.status {
            TestStatus::Generated => (text::MUTED, bg::ELEVATED, "Generated"),
            TestStatus::Running => (status::RUNNING, status::INFO_BG, "Running"),
            TestStatus::Passed => (status::SUCCESS, status::SUCCESS_BG, "Passed"),
            TestStatus::Failed => (status::ERROR, status::ERROR_BG, "Failed"),
            TestStatus::Skipped => (text::DISABLED, bg::ELEVATED, "Skipped"),
        };

        let name = test.name.clone();
        let category = test.category.label().to_string();

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Name and status
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
                            .font_weight(FontWeight::MEDIUM)
                            .child(name)
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .bg(status_bg)
                            .rounded(px(4.0))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status_color)
                            .font_weight(FontWeight::MEDIUM)
                            .child(status_text)
                    )
            )
            // Category and confidence
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
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
                                    .child("Category:")
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .child(category)
                            )
                    )
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
                                    .child("Confidence:")
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .child(format!("{:.0}%", test.confidence * 100.0))
                            )
                    )
            )
    }

    fn render_description(&self, test: &TestInfo) -> impl IntoElement {
        let description = test.description.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::SUBTLE)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Description")
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .line_height(px(18.0))
                    .child(description)
            )
    }

    fn render_code(&self, test: &TestInfo) -> impl IntoElement {
        let code = test.code.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::SUBTLE)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Test Code")
            )
            .child(
                div()
                    .p(px(12.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(border::SUBTLE)
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family("Berkeley Mono")
                            .text_color(text::PRIMARY)
                            .line_height(px(16.0))
                            .child(code)
                    )
            )
    }

    fn render_reasoning(&self, test: &TestInfo) -> impl IntoElement {
        let reasoning = test.reasoning.clone().unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Generation Reasoning")
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::SECONDARY)
                    .line_height(px(18.0))
                    .child(reasoning)
            )
    }
}

impl Focusable for TestDetail {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestDetail {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("test-detail-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .bg(bg::SURFACE)
            .when(self.test.is_none(), |el| {
                el.flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .text_color(text::DISABLED)
                                    .child("◇")
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Select a test to view details")
                            )
                    )
            })
            .when_some(self.test.as_ref(), |el, test| {
                el
                    .child(self.render_header(test))
                    .child(self.render_description(test))
                    .child(self.render_code(test))
                    .when(test.reasoning.is_some(), |el| {
                        el.child(self.render_reasoning(test))
                    })
            })
    }
}
