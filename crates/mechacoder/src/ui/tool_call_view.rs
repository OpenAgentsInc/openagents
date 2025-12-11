//! Tool call view component.

use acp::{ToolCall, ToolCallContent, ToolCallStatus};
use gpui::{div, prelude::*, px, IntoElement, ParentElement, Styled};
use theme::{bg, border, status, text};

/// Tool call view for displaying a tool call.
pub struct ToolCallView {
    /// Tool call ID.
    id: String,
    /// Tool call title.
    title: String,
    /// Tool call status.
    status: ToolCallStatus,
    /// Tool call content.
    content: Vec<ToolCallContent>,
    /// Whether the view is expanded.
    expanded: bool,
}

impl ToolCallView {
    /// Create a new tool call view.
    pub fn new(tool_call: &ToolCall) -> Self {
        Self {
            id: tool_call.id.to_string(),
            title: tool_call.title.clone(),
            status: tool_call.status.clone(),
            content: tool_call.content.clone(),
            expanded: true,
        }
    }

    /// Get the status color.
    fn status_color(&self) -> gpui::Hsla {
        match &self.status {
            ToolCallStatus::Pending => status::PENDING,
            ToolCallStatus::InProgress => status::RUNNING,
            ToolCallStatus::Completed => status::SUCCESS,
            ToolCallStatus::Failed { .. } => status::ERROR,
            ToolCallStatus::Rejected => status::WARNING,
            ToolCallStatus::Canceled => status::WARNING,
            ToolCallStatus::WaitingForConfirmation { .. } => status::INFO,
        }
    }

    /// Get the status text.
    fn status_text(&self) -> &str {
        match &self.status {
            ToolCallStatus::Pending => "Pending",
            ToolCallStatus::InProgress => "Running",
            ToolCallStatus::Completed => "Completed",
            ToolCallStatus::Failed { .. } => "Failed",
            ToolCallStatus::Rejected => "Rejected",
            ToolCallStatus::Canceled => "Canceled",
            ToolCallStatus::WaitingForConfirmation { .. } => "Waiting",
        }
    }

    /// Render a single content item.
    fn render_content_item(item: &ToolCallContent) -> impl IntoElement {
        match item {
            ToolCallContent::Text(text) => div()
                .p(px(8.0))
                .bg(bg::CODE)
                .rounded(px(4.0))
                .text_sm()
                .text_color(text::PRIMARY)
                .child(text.clone())
                .into_any_element(),

            ToolCallContent::Diff {
                path,
                old_content,
                new_content,
            } => {
                // Simple diff display
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(text::PRIMARY)
                            .child(format!("File: {}", path.display())),
                    )
                    .child(
                        div()
                            .p(px(8.0))
                            .bg(bg::CODE)
                            .rounded(px(4.0))
                            .text_sm()
                            .text_color(text::SECONDARY)
                            .child(format!(
                                "-{} lines, +{} lines",
                                old_content.lines().count(),
                                new_content.lines().count()
                            )),
                    )
                    .into_any_element()
            }

            ToolCallContent::Terminal { terminal_id, output } => div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(text::SECONDARY)
                        .child(format!("Terminal: {}", terminal_id)),
                )
                .child(
                    div()
                        .p(px(8.0))
                        .bg(bg::DARK)
                        .rounded(px(4.0))
                        .text_sm()
                        .text_color(text::PRIMARY)
                        .max_h(px(200.0))
                        .overflow_y_scroll()
                        .child(output.clone()),
                )
                .into_any_element(),
        }
    }
}

impl IntoElement for ToolCallView {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        div()
            .px(px(16.0))
            .py(px(8.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    // Status indicator
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(self.status_color()),
                    )
                    // Title
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text::PRIMARY)
                            .child(self.title),
                    )
                    // Status text
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::SECONDARY)
                            .child(self.status_text()),
                    ),
            )
            // Content (if expanded)
            .when(self.expanded && !self.content.is_empty(), |el| {
                el.child(
                    div()
                        .pl(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(self.content.iter().map(Self::render_content_item)),
                )
            })
    }
}
