//! MessageBubble widget - renders a single chat message.
//!
//! Supports markdown rendering and role-based styling.

use coder_domain::message::Role;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget, WidgetId};
use wgpui::markdown::{MarkdownParser, MarkdownRenderer, StreamingMarkdown};
use wgpui::{Bounds, Hsla, InputEvent, Point, Quad};

/// A chat message bubble widget.
pub struct MessageBubble {
    /// Widget ID.
    id: Option<WidgetId>,

    /// Message content (markdown).
    content: String,

    /// Message role (user/assistant/system).
    role: Role,

    /// Whether this message is currently streaming.
    streaming: bool,

    /// Font size.
    font_size: f32,

    /// Padding.
    padding: f32,

    /// Corner radius.
    corner_radius: f32,

    /// Streaming markdown state (for streaming messages).
    streaming_md: Option<StreamingMarkdown>,
}

impl MessageBubble {
    /// Create a new message bubble.
    pub fn new(content: &str, role: Role) -> Self {
        Self {
            id: None,
            content: content.to_string(),
            role,
            streaming: false,
            font_size: 14.0,
            padding: 12.0,
            corner_radius: 8.0,
            streaming_md: None,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set streaming state.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        if streaming && self.streaming_md.is_none() {
            let mut md = StreamingMarkdown::new();
            md.append(&self.content);
            self.streaming_md = Some(md);
        }
        self
    }

    /// Get the background color based on role.
    fn background_color(&self) -> Hsla {
        match self.role {
            Role::User => Hsla::transparent(),
            Role::Assistant => Hsla::transparent(),
            Role::System => wgpui::theme::bg::CODE,
        }
    }

    /// Get the border color based on role.
    fn border_color(&self) -> Hsla {
        match self.role {
            Role::User => Hsla::transparent(),
            Role::Assistant => Hsla::transparent(),
            Role::System => wgpui::theme::border::SUBTLE,
        }
    }

    /// Tick the streaming animation.
    pub fn tick(&mut self) {
        if let Some(md) = &mut self.streaming_md {
            md.tick();
        }
    }

    /// Append content (for streaming).
    pub fn append(&mut self, text: &str) {
        self.content.push_str(text);
        if let Some(md) = &mut self.streaming_md {
            md.append(text);
        }
    }

    /// Mark streaming as complete.
    pub fn complete(&mut self) {
        if let Some(md) = &mut self.streaming_md {
            md.complete();
        }
    }
}

impl Widget for MessageBubble {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw bubble background (skip for transparent user messages)
        let bg = self.background_color();
        if bg.a > 0.0 {
            cx.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(bg)
                    .with_border(self.border_color(), 1.0)
                    .with_uniform_radius(self.corner_radius),
            );
        }

        // Content area (no label)
        let content_y = bounds.origin.y + self.padding;
        let content_width = bounds.size.width - self.padding * 2.0;

        // Render markdown content
        if let Some(streaming_md) = &self.streaming_md {
            // Use streaming renderer with fade-in
            let renderer = MarkdownRenderer::new();
            let fade = streaming_md.fade_state();
            renderer.render_with_opacity(
                streaming_md.document(),
                Point::new(bounds.origin.x + self.padding, content_y),
                content_width,
                cx.text,
                &mut cx.scene,
                fade.new_content_opacity,
            );
        } else {
            // Static markdown render
            let parser = MarkdownParser::new();
            let renderer = MarkdownRenderer::new();
            let doc = parser.parse(&self.content);
            renderer.render(
                &doc,
                Point::new(bounds.origin.x + self.padding, content_y),
                content_width,
                cx.text,
                &mut cx.scene,
            );
        }

        // Streaming indicator
        if self.streaming {
            let indicator_x = bounds.origin.x + bounds.size.width - self.padding - 8.0;
            let indicator_y = bounds.origin.y + self.padding;

            // Simple pulsing dot
            cx.scene.draw_quad(
                Quad::new(Bounds::new(indicator_x, indicator_y, 8.0, 8.0))
                    .with_background(wgpui::theme::accent::PRIMARY)
                    .with_uniform_radius(4.0),
            );
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        // Messages don't handle events currently
        // TODO: Add selection, copy, etc.
        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Approximate height based on content lines
        let line_count = self.content.lines().count().max(1);
        let content_height = line_count as f32 * (self.font_size * 1.4);
        let total_height = content_height + self.padding * 2.0;
        (None, Some(total_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_bubble_creation() {
        let bubble = MessageBubble::new("Hello, world!", Role::User).id(1);

        assert_eq!(bubble.id, Some(1));
        assert_eq!(bubble.content, "Hello, world!");
        assert_eq!(bubble.role, Role::User);
        assert!(!bubble.streaming);
    }

    #[test]
    fn test_message_bubble_streaming() {
        let mut bubble = MessageBubble::new("Hello", Role::Assistant).streaming(true);

        assert!(bubble.streaming);
        assert!(bubble.streaming_md.is_some());

        bubble.append(", world!");
        assert_eq!(bubble.content, "Hello, world!");
    }

    #[test]
    fn test_size_hint() {
        let bubble = MessageBubble::new("Line 1\nLine 2\nLine 3", Role::User);
        let (_, height) = bubble.size_hint();

        assert!(height.is_some());
        assert!(height.unwrap() > 0.0);
    }
}
