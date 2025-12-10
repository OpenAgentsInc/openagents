//! Thread Item types and rendering
//!
//! Ported from Effuse's atif-thread.ts

use chrono::{DateTime, Utc};
use gpui::*;
use theme::{accent, bg, border, category, status, text, FONT_FAMILY};

// ============================================================================
// Types (from atif-thread.ts)
// ============================================================================

/// Test category for categorization badges
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestCategory {
    AntiCheat,
    Existence,
    Correctness,
    Boundary,
    Integration,
    Other(String),
}

impl TestCategory {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anti_cheat" | "anticheat" => Self::AntiCheat,
            "existence" => Self::Existence,
            "correctness" => Self::Correctness,
            "boundary" => Self::Boundary,
            "integration" => Self::Integration,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::AntiCheat => "anti_cheat",
            Self::Existence => "existence",
            Self::Correctness => "correctness",
            Self::Boundary => "boundary",
            Self::Integration => "integration",
            Self::Other(s) => s,
        }
    }

    /// Get badge styling (background, text, border colors)
    pub fn badge_colors(&self) -> (Hsla, Hsla, Hsla) {
        match self {
            Self::AntiCheat => (category::ANTI_CHEAT_BG, category::ANTI_CHEAT_TEXT, category::ANTI_CHEAT_BORDER),
            Self::Existence => (category::EXISTENCE_BG, category::EXISTENCE_TEXT, category::EXISTENCE_BORDER),
            Self::Correctness => (category::CORRECTNESS_BG, category::CORRECTNESS_TEXT, category::CORRECTNESS_BORDER),
            Self::Boundary => (category::BOUNDARY_BG, category::BOUNDARY_TEXT, category::BOUNDARY_BORDER),
            Self::Integration => (category::INTEGRATION_BG, category::INTEGRATION_TEXT, category::INTEGRATION_BORDER),
            Self::Other(_) => (category::UNKNOWN_BG, category::UNKNOWN_TEXT, category::UNKNOWN_BORDER),
        }
    }
}

/// Reflection action type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectionAction {
    Refining,
    Assessing,
    Complete,
}

impl ReflectionAction {
    pub fn label(&self) -> &str {
        match self {
            Self::Refining => "Refining",
            Self::Assessing => "Assessing",
            Self::Complete => "Complete",
        }
    }
}

/// Progress data for progress items
#[derive(Debug, Clone)]
pub struct ProgressData {
    pub phase: String,
    pub category: Option<String>,
    pub round: i32,
    pub status: String,
}

/// Reflection data
#[derive(Debug, Clone)]
pub struct ReflectionData {
    pub category: Option<String>,
    pub text: String,
    pub action: ReflectionAction,
}

/// Test data
#[derive(Debug, Clone)]
pub struct TestData {
    pub id: String,
    pub category: String,
    pub input: String,
    pub expected_output: Option<String>,
    pub reasoning: String,
    pub confidence: f32,
}

/// Completion data
#[derive(Debug, Clone)]
pub struct CompleteData {
    pub total_tests: i32,
    pub total_rounds: i32,
    pub comprehensiveness_score: Option<f32>,
    pub total_tokens_used: i64,
    pub duration_ms: i64,
    pub uncertainties: Vec<String>,
}

/// Error data
#[derive(Debug, Clone)]
pub struct ErrorData {
    pub error: String,
}

/// Thread item types (from atif-thread.ts ThreadItem)
#[derive(Debug, Clone)]
pub enum ThreadItem {
    Progress {
        timestamp: DateTime<Utc>,
        data: ProgressData,
    },
    Reflection {
        timestamp: DateTime<Utc>,
        data: ReflectionData,
    },
    Test {
        timestamp: DateTime<Utc>,
        data: TestData,
    },
    Complete {
        timestamp: DateTime<Utc>,
        data: CompleteData,
    },
    Error {
        timestamp: DateTime<Utc>,
        data: ErrorData,
    },
}

impl ThreadItem {
    /// Get a unique ID for the thread item
    pub fn id(&self) -> String {
        match self {
            Self::Test { data, .. } => data.id.clone(),
            Self::Progress { timestamp, .. } => format!("progress-{}", timestamp.timestamp_millis()),
            Self::Reflection { timestamp, .. } => format!("reflection-{}", timestamp.timestamp_millis()),
            Self::Complete { timestamp, .. } => format!("complete-{}", timestamp.timestamp_millis()),
            Self::Error { timestamp, .. } => format!("error-{}", timestamp.timestamp_millis()),
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Progress { timestamp, .. }
            | Self::Reflection { timestamp, .. }
            | Self::Test { timestamp, .. }
            | Self::Complete { timestamp, .. }
            | Self::Error { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// Rendering
// ============================================================================

/// Format timestamp as HH:MM:SS
fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.format("%H:%M:%S").to_string()
}

/// Render a confidence bar
pub fn render_confidence_bar(confidence: f32) -> impl IntoElement {
    let percent = (confidence * 100.0).round() as i32;
    let _width_percent = format!("{}%", percent);

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .flex_1()
                .h(px(8.0))
                .bg(bg::CARD)
                .rounded(px(4.0))
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .bg(status::SUCCESS)
                        .w(relative(confidence)),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(text::SECONDARY)
                .font_family(FONT_FAMILY)
                .child(format!("{}%", percent)),
        )
}

/// Render a category badge
pub fn render_category_badge(category: &TestCategory) -> impl IntoElement {
    let (bg, text, border) = category.badge_colors();
    let label = category.as_str().to_string();

    div()
        .px(px(8.0))
        .py(px(4.0))
        .text_size(px(12.0))
        .font_family(FONT_FAMILY)
        .bg(bg)
        .text_color(text)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .flex_shrink_0()
        .child(label)
}

/// Render a progress item
pub fn render_progress_item(timestamp: DateTime<Utc>, data: &ProgressData) -> impl IntoElement {
    div()
        .p(px(12.0))
        .bg(bg::HOVER)
        .border_1()
        .border_color(border::STRONG)
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text::MUTED)
                        .font_family(FONT_FAMILY)
                        .child(format_timestamp(timestamp)),
                )
                .child(div().text_color(text::SECONDARY).child("⚙️"))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .child("PROGRESS"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .child(data.status.clone()),
                )
                .children(data.category.as_ref().map(|cat| {
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("({} - round {})", cat, data.round))
                })),
        )
}

/// Render a reflection item
pub fn render_reflection_item(timestamp: DateTime<Utc>, data: &ReflectionData) -> impl IntoElement {
    let action_label = data.action.label().to_string();
    let reflection_text = data.text.clone();
    let reflection_category = data.category.clone();

    div()
        .p(px(12.0))
        .bg(status::INFO_BG)
        .border_1()
        .border_color(status::INFO_BORDER)
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text::MUTED)
                        .font_family(FONT_FAMILY)
                        .child(format_timestamp(timestamp)),
                )
                .child(div().text_color(accent::PRIMARY).child("💭"))
                .child(
                    div()
                        .flex_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .mb(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(accent::PRIMARY)
                                        .child("REFLECTION"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status::INFO)
                                        .child(action_label),
                                )
                                .children(reflection_category.map(|cat| {
                                    div()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .bg(status::INFO_BG)
                                        .border_1()
                                        .border_color(status::INFO_BORDER)
                                        .rounded(px(4.0))
                                        .text_size(px(12.0))
                                        .text_color(accent::PRIMARY)
                                        .font_family(FONT_FAMILY)
                                        .child(cat)
                                })),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(accent::PRIMARY)
                                .font_family(FONT_FAMILY)
                                .line_height(px(22.0))
                                .child(reflection_text),
                        ),
                ),
        )
}

/// Render an error item
pub fn render_error_item(timestamp: DateTime<Utc>, data: &ErrorData) -> impl IntoElement {
    div()
        .p(px(12.0))
        .bg(status::ERROR_BG)
        .border_1()
        .border_color(status::ERROR_BORDER)
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text::MUTED)
                        .font_family(FONT_FAMILY)
                        .child(format_timestamp(timestamp)),
                )
                .child(div().text_color(status::ERROR).child("✗"))
                .child(
                    div()
                        .flex_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .mb(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status::ERROR)
                                        .child("ERROR"),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(status::ERROR)
                                .font_family(FONT_FAMILY)
                                .child(data.error.clone()),
                        ),
                ),
        )
}

/// Render a complete item
pub fn render_complete_item(timestamp: DateTime<Utc>, data: &CompleteData) -> impl IntoElement {
    div()
        .p(px(16.0))
        .bg(status::SUCCESS_BG)
        .border_1()
        .border_color(status::SUCCESS_BORDER)
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .mb(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text::MUTED)
                        .font_family(FONT_FAMILY)
                        .child(format_timestamp(timestamp)),
                )
                .child(div().text_color(status::SUCCESS).child("✓"))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::SUCCESS)
                        .child("COMPLETE"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .text_size(px(14.0))
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(div().text_color(text::SECONDARY).font_family(FONT_FAMILY).child("Total Tests:"))
                        .child(
                            div()
                                .text_color(status::SUCCESS)
                                .font_family(FONT_FAMILY)
                                .child(format!("{}", data.total_tests)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(div().text_color(text::SECONDARY).font_family(FONT_FAMILY).child("Total Rounds:"))
                        .child(
                            div()
                                .text_color(status::SUCCESS)
                                .font_family(FONT_FAMILY)
                                .child(format!("{}", data.total_rounds)),
                        ),
                )
                .children(data.comprehensiveness_score.map(|score| {
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_color(text::SECONDARY)
                                .font_family(FONT_FAMILY)
                                .child("Comprehensiveness Score:"),
                        )
                        .child(
                            div()
                                .text_color(status::SUCCESS)
                                .font_family(FONT_FAMILY)
                                .child(format!("{}/10", score)),
                        )
                }))
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(div().text_color(text::SECONDARY).font_family(FONT_FAMILY).child("Tokens Used:"))
                        .child(
                            div()
                                .text_color(status::SUCCESS)
                                .font_family(FONT_FAMILY)
                                .child(format!("{}", data.total_tokens_used)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .child(div().text_color(text::SECONDARY).font_family(FONT_FAMILY).child("Duration:"))
                        .child(
                            div()
                                .text_color(status::SUCCESS)
                                .font_family(FONT_FAMILY)
                                .child(format!("{:.1}s", data.duration_ms as f64 / 1000.0)),
                        ),
                ),
        )
}

/// Render a test item (header only, not expanded)
pub fn render_test_item_header(
    timestamp: DateTime<Utc>,
    data: &TestData,
    is_expanded: bool,
) -> impl IntoElement {
    let test_category = TestCategory::from_str(&data.category);
    let percent = (data.confidence * 100.0).round() as i32;

    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(12.0))
        .bg(bg::HOVER)
        .border_1()
        .border_color(border::STRONG)
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::CARD))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text::MUTED)
                        .font_family(FONT_FAMILY)
                        .flex_shrink_0()
                        .child(format_timestamp(timestamp)),
                )
                .child(render_category_badge(&test_category))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(text::PRIMARY)
                        .font_family(FONT_FAMILY)
                        .truncate()
                        .child(data.id.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .flex_shrink_0()
                        .child(format!("({}%)", percent)),
                ),
        )
        .child(
            div()
                .text_color(text::MUTED)
                .flex_shrink_0()
                .ml(px(8.0))
                .child(if is_expanded { "▲" } else { "▼" }),
        )
}

/// Render test item details (expanded view)
pub fn render_test_item_details(data: &TestData) -> impl IntoElement {
    div()
        .mt(px(8.0))
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        // Input
        .child(
            div()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(4.0))
                        .child("INPUT"),
                )
                .child(
                    div()
                        .p(px(8.0))
                        .bg(bg::SURFACE)
                        .rounded(px(4.0))
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::SUCCESS)
                        .overflow_hidden()
                        .child(data.input.clone()),
                ),
        )
        // Expected Output (if any)
        .children(data.expected_output.as_ref().map(|output| {
            div()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(4.0))
                        .child("EXPECTED OUTPUT"),
                )
                .child(
                    div()
                        .p(px(8.0))
                        .bg(bg::SURFACE)
                        .rounded(px(4.0))
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::PRIMARY)
                        .overflow_hidden()
                        .child(output.clone()),
                )
        }))
        // Reasoning
        .child(
            div()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(4.0))
                        .child("REASONING"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::SECONDARY)
                        .line_height(px(22.0))
                        .child(data.reasoning.clone()),
                ),
        )
        // Confidence
        .child(
            div()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(4.0))
                        .child("CONFIDENCE"),
                )
                .child(render_confidence_bar(data.confidence)),
        )
}

/// Render a full thread item
pub fn render_thread_item(item: &ThreadItem, is_expanded: bool) -> impl IntoElement {
    match item {
        ThreadItem::Progress { timestamp, data } => render_progress_item(*timestamp, data).into_any_element(),
        ThreadItem::Reflection { timestamp, data } => render_reflection_item(*timestamp, data).into_any_element(),
        ThreadItem::Error { timestamp, data } => render_error_item(*timestamp, data).into_any_element(),
        ThreadItem::Complete { timestamp, data } => render_complete_item(*timestamp, data).into_any_element(),
        ThreadItem::Test { timestamp, data } => {
            if is_expanded {
                div()
                    .child(render_test_item_header(*timestamp, data, true))
                    .child(render_test_item_details(data))
                    .into_any_element()
            } else {
                render_test_item_header(*timestamp, data, false).into_any_element()
            }
        }
    }
}
