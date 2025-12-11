//! Tool call view component.

use crate::sdk_thread::{ToolUse, ToolStatus};
use gpui::{div, prelude::*, px, IntoElement, ParentElement, Styled};
use theme_oa::{bg, status, text};

/// Simple content for tool call display.
#[derive(Clone)]
pub enum ToolCallContent {
    /// Plain text content.
    Text(String),
}

/// Tool call view for displaying a tool call.
pub struct ToolCallView {
    /// Tool call title.
    title: String,
    /// Tool call status.
    tool_status: ToolStatus,
    /// Tool call content.
    content: Vec<ToolCallContent>,
    /// Whether the view is expanded.
    expanded: bool,
}

impl ToolCallView {
    /// Create a new tool call view from SDK ToolUse.
    pub fn from_tool_use(tool_use: &ToolUse) -> Self {
        let mut content = Vec::new();

        // Add input as content
        if !tool_use.input.is_empty() {
            content.push(ToolCallContent::Text(format!("Input: {}", tool_use.input)));
        }

        // Add output if available
        if let Some(output) = &tool_use.output {
            content.push(ToolCallContent::Text(format!("Output: {}", output)));
        }

        Self {
            title: tool_use.tool_name.clone(),
            tool_status: tool_use.status.clone(),
            content,
            expanded: true,
        }
    }

    /// Get the status color.
    fn status_color(&self) -> gpui::Hsla {
        match &self.tool_status {
            ToolStatus::Pending => status::PENDING,
            ToolStatus::Running => status::RUNNING,
            ToolStatus::Completed => status::SUCCESS,
            ToolStatus::Failed(_) => status::ERROR,
        }
    }

    /// Get the status text.
    fn status_text(&self) -> &str {
        match &self.tool_status {
            ToolStatus::Pending => "Pending",
            ToolStatus::Running => "Running",
            ToolStatus::Completed => "Completed",
            ToolStatus::Failed(_) => "Failed",
        }
    }

    /// Render a single content item.
    fn render_content_item(item: &ToolCallContent) -> impl IntoElement {
        match item {
            ToolCallContent::Text(text_content) => div()
                .p(px(8.0))
                .bg(bg::CODE)
                .rounded(px(4.0))
                .text_sm()
                .text_color(text::PRIMARY)
                .child(text_content.clone()),
        }
    }
}

impl IntoElement for ToolCallView {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        // Extract values before consuming self
        let status_color = self.status_color();
        let status_text = self.status_text().to_string();
        let title = self.title;
        let expanded = self.expanded;
        let content = self.content;

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
                            .bg(status_color),
                    )
                    // Title
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(text::PRIMARY)
                            .child(title),
                    )
                    // Status text
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::SECONDARY)
                            .child(status_text),
                    ),
            )
            // Content (if expanded)
            .when(expanded && !content.is_empty(), |el| {
                el.child(
                    div()
                        .pl(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(content.iter().map(Self::render_content_item)),
                )
            })
    }
}
