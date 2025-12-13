//! Tool call view component.

use crate::sdk_thread::{ToolStatus, ToolUse};
use gpui::{div, prelude::*, px, App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use theme_oa::{bg, status, text};

/// Tool call view for displaying a tool call (collapsible).
pub struct ToolCallView {
    /// Tool call title.
    title: String,
    /// Tool call status.
    tool_status: ToolStatus,
    /// Input content.
    input: String,
    /// Output content.
    output: Option<String>,
    /// Whether the view is expanded.
    expanded: bool,
}

impl ToolCallView {
    /// Create a new tool call view from SDK ToolUse.
    pub fn from_tool_use(tool_use: &ToolUse, cx: &mut App) -> Entity<Self> {
        cx.new(|_cx| Self {
            title: tool_use.tool_name.clone(),
            tool_status: tool_use.status.clone(),
            input: tool_use.input.clone(),
            output: tool_use.output.clone(),
            expanded: false, // Collapsed by default
        })
    }

    /// Update from a ToolUse (status and output may have changed).
    pub fn update_from(&mut self, tool_use: &ToolUse) {
        self.tool_status = tool_use.status.clone();
        self.output = tool_use.output.clone();
    }

    /// Get current status for comparison.
    pub fn status(&self) -> &ToolStatus {
        &self.tool_status
    }

    /// Toggle expanded state.
    fn toggle_expanded(&mut self, _cx: &mut Context<Self>) {
        self.expanded = !self.expanded;
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
            ToolStatus::Pending => "pending",
            ToolStatus::Running => "running...",
            ToolStatus::Completed => "done",
            ToolStatus::Failed(_) => "failed",
        }
    }

    /// Get the expand/collapse indicator.
    fn expand_indicator(&self) -> String {
        if self.expanded { "▼".to_string() } else { "▶".to_string() }
    }
}

impl Render for ToolCallView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_color = self.status_color();
        let status_text = self.status_text().to_string();
        let title = self.title.clone();
        let expanded = self.expanded;
        let input = self.input.clone();
        let output = self.output.clone();
        let indicator = self.expand_indicator();

        div()
            .px(px(16.0))
            .py(px(4.0))
            .flex()
            .flex_col()
            // Header (clickable)
            .child(
                div()
                    .id("tool-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.toggle_expanded(cx);
                        cx.notify();
                    }))
                    // Expand indicator
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::SECONDARY)
                            .child(indicator),
                    )
                    // Status indicator dot
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .bg(status_color),
                    )
                    // Title
                    .child(
                        div()
                            .text_sm()
                            .text_color(text::SECONDARY)
                            .child(title),
                    )
                    // Status text
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(status_text),
                    ),
            )
            // Content (only if expanded)
            .when(expanded, |el| {
                el.child(
                    div()
                        .pl(px(24.0))
                        .pt(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        // Input
                        .when(!input.is_empty(), |el| {
                            el.child(
                                div()
                                    .p(px(8.0))
                                    .bg(bg::CODE)
                                    .text_xs()
                                    .text_color(text::SECONDARY)
                                    .overflow_hidden()
                                    .child(format!("Input: {}", input)),
                            )
                        })
                        // Output
                        .when_some(output, |el, out| {
                            el.child(
                                div()
                                    .p(px(8.0))
                                    .bg(bg::CODE)
                                    .text_xs()
                                    .text_color(text::SECONDARY)
                                    .overflow_hidden()
                                    .child(format!("Output: {}", out)),
                            )
                        }),
                )
            })
    }
}
