//! ToolUseIndicator widget - shows tool execution status.
//!
//! Displays a compact indicator for tool use within messages.

use coder_domain::tool::ToolUseStatus;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget, WidgetId};
use wgpui::{Bounds, Hsla, InputEvent, Point, Quad};

/// A tool use indicator widget.
pub struct ToolUseIndicator {
    /// Widget ID.
    id: Option<WidgetId>,

    /// Tool name.
    tool_name: String,

    /// Current status.
    status: ToolUseStatus,

    /// Input summary (optional).
    input_summary: Option<String>,

    /// Result summary (optional).
    result_summary: Option<String>,

    /// Duration in milliseconds.
    duration_ms: Option<u64>,

    /// Whether expanded to show details.
    expanded: bool,

    /// Font size.
    font_size: f32,

    /// Padding.
    padding: f32,
}

impl ToolUseIndicator {
    /// Create a new tool use indicator.
    pub fn new(tool_name: &str, status: ToolUseStatus) -> Self {
        Self {
            id: None,
            tool_name: tool_name.to_string(),
            status,
            input_summary: None,
            result_summary: None,
            duration_ms: None,
            expanded: false,
            font_size: 12.0,
            padding: 8.0,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set input summary.
    pub fn input_summary(mut self, summary: impl Into<String>) -> Self {
        self.input_summary = Some(summary.into());
        self
    }

    /// Set result summary.
    pub fn result_summary(mut self, summary: impl Into<String>) -> Self {
        self.result_summary = Some(summary.into());
        self
    }

    /// Set duration.
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Toggle expanded state.
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Get status color.
    fn status_color(&self) -> Hsla {
        match self.status {
            ToolUseStatus::Pending => wgpui::theme::text::MUTED,
            ToolUseStatus::Running => wgpui::theme::status::RUNNING,
            ToolUseStatus::Success => wgpui::theme::status::SUCCESS,
            ToolUseStatus::Failed => wgpui::theme::status::ERROR,
            ToolUseStatus::Cancelled => wgpui::theme::text::DISABLED,
        }
    }

    /// Get status icon (as text for now).
    fn status_icon(&self) -> &'static str {
        match self.status {
            ToolUseStatus::Pending => "...",
            ToolUseStatus::Running => "...",
            ToolUseStatus::Success => "[ok]",
            ToolUseStatus::Failed => "[x]",
            ToolUseStatus::Cancelled => "[--]",
        }
    }

    /// Get background color.
    fn background_color(&self) -> Hsla {
        wgpui::theme::bg::CODE
    }
}

impl Widget for ToolUseIndicator {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.background_color())
                .with_border(wgpui::theme::border::SUBTLE, 1.0)
                .with_uniform_radius(4.0),
        );

        // Status indicator (left side)
        let status_x = bounds.origin.x + self.padding;
        let status_y = bounds.origin.y + self.padding;

        let status_run = cx.text.layout(
            self.status_icon(),
            Point::new(status_x, status_y),
            self.font_size,
            self.status_color(),
        );
        cx.scene.draw_text(status_run);

        // Tool name
        let name_x = status_x + 30.0;
        let name_run = cx.text.layout(
            &self.tool_name,
            Point::new(name_x, status_y),
            self.font_size,
            wgpui::theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Duration (if complete)
        if let Some(ms) = self.duration_ms {
            let duration_text = format!("{}ms", ms);
            let duration_x = bounds.origin.x + bounds.size.width - self.padding - 50.0;

            let duration_run = cx.text.layout(
                &duration_text,
                Point::new(duration_x, status_y),
                self.font_size,
                wgpui::theme::text::MUTED,
            );
            cx.scene.draw_text(duration_run);
        }

        // Expanded details
        if self.expanded {
            let details_y = status_y + self.font_size + 8.0;

            // Input summary
            if let Some(input) = &self.input_summary {
                let input_label = cx.text.layout(
                    "Input: ",
                    Point::new(status_x, details_y),
                    self.font_size,
                    wgpui::theme::text::MUTED,
                );
                cx.scene.draw_text(input_label);

                let input_text = cx.text.layout(
                    input,
                    Point::new(status_x + 40.0, details_y),
                    self.font_size,
                    wgpui::theme::text::SECONDARY,
                );
                cx.scene.draw_text(input_text);
            }

            // Result summary
            if let Some(result) = &self.result_summary {
                let result_y = details_y + self.font_size + 4.0;

                let result_label = cx.text.layout(
                    "Result: ",
                    Point::new(status_x, result_y),
                    self.font_size,
                    wgpui::theme::text::MUTED,
                );
                cx.scene.draw_text(result_label);

                let result_text = cx.text.layout(
                    result,
                    Point::new(status_x + 50.0, result_y),
                    self.font_size,
                    if self.status == ToolUseStatus::Success {
                        wgpui::theme::text::SECONDARY
                    } else {
                        wgpui::theme::status::ERROR
                    },
                );
                cx.scene.draw_text(result_text);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        // Toggle expanded on click
        match event {
            InputEvent::MouseDown { position, .. } => {
                if bounds.contains(*position) {
                    self.toggle_expanded();
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let base_height = self.font_size + self.padding * 2.0;
        let height = if self.expanded {
            base_height + (self.font_size + 4.0) * 2.0 // Two extra lines
        } else {
            base_height
        };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_indicator_creation() {
        let indicator = ToolUseIndicator::new("read_file", ToolUseStatus::Running)
            .id(1)
            .input_summary("{path: '/foo/bar'}")
            .duration_ms(150);

        assert_eq!(indicator.id, Some(1));
        assert_eq!(indicator.tool_name, "read_file");
        assert_eq!(indicator.status, ToolUseStatus::Running);
        assert!(indicator.input_summary.is_some());
        assert_eq!(indicator.duration_ms, Some(150));
    }

    #[test]
    fn test_status_colors() {
        let pending = ToolUseIndicator::new("test", ToolUseStatus::Pending);
        let running = ToolUseIndicator::new("test", ToolUseStatus::Running);
        let success = ToolUseIndicator::new("test", ToolUseStatus::Success);
        let failed = ToolUseIndicator::new("test", ToolUseStatus::Failed);

        // Just verify they return valid colors
        assert!(pending.status_color().a > 0.0);
        assert!(running.status_color().a > 0.0);
        assert!(success.status_color().a > 0.0);
        assert!(failed.status_color().a > 0.0);
    }

    #[test]
    fn test_toggle_expanded() {
        let mut indicator = ToolUseIndicator::new("test", ToolUseStatus::Success);

        assert!(!indicator.expanded);
        indicator.toggle_expanded();
        assert!(indicator.expanded);
        indicator.toggle_expanded();
        assert!(!indicator.expanded);
    }

    #[test]
    fn test_size_hint() {
        let collapsed = ToolUseIndicator::new("test", ToolUseStatus::Success);
        let (_, collapsed_height) = collapsed.size_hint();

        let mut expanded = ToolUseIndicator::new("test", ToolUseStatus::Success);
        expanded.expanded = true;
        let (_, expanded_height) = expanded.size_hint();

        assert!(expanded_height.unwrap() > collapsed_height.unwrap());
    }
}
