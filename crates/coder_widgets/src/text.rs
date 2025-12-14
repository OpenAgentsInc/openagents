//! Text widget - renders text content.
//!
//! The Text widget displays styled text using the text system.

use crate::context::{EventContext, PaintContext};
use crate::widget::{EventResult, Widget, WidgetId};
use wgpui::{Bounds, Hsla, InputEvent, Point};

/// A text widget that renders a string.
pub struct Text {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Text content.
    content: String,
    /// Font size.
    font_size: f32,
    /// Text color.
    color: Hsla,
}

impl Text {
    /// Create a new Text widget.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            font_size: 14.0,
            color: wgpui::theme::text::PRIMARY,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the text color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Update the text content.
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }

    /// Get the text content.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Widget for Text {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.content.is_empty() {
            return;
        }

        // Render text
        let text_run = cx.text.layout(
            &self.content,
            Point::new(bounds.origin.x, bounds.origin.y),
            self.font_size,
            self.color,
        );
        cx.scene.draw_text(text_run);
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        // Text doesn't handle events by default
        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Approximate text size
        let width = self.content.len() as f32 * self.font_size * 0.6;
        let height = self.font_size * 1.4;
        (Some(width), Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_creation() {
        let text = Text::new("Hello, world!")
            .font_size(16.0)
            .color(wgpui::theme::text::SECONDARY);

        assert_eq!(text.content, "Hello, world!");
        assert_eq!(text.font_size, 16.0);
    }

    #[test]
    fn test_text_update() {
        let mut text = Text::new("Original");
        assert_eq!(text.content(), "Original");

        text.set_content("Updated");
        assert_eq!(text.content(), "Updated");
    }
}
