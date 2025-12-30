//! Text component for rendering styled text with optional wrapping.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::styled::{StyleRefinement, Styled};
use crate::text::FontStyle;
use crate::text_system::{LineFragment, LineWrapper};
use crate::{Bounds, Hsla, InputEvent, Point, theme};

/// A component that renders text content with optional word wrapping.
pub struct Text {
    id: Option<ComponentId>,
    content: String,
    pub(crate) style: StyleRefinement,
    font_size: f32,
    color: Hsla,
    font_style: FontStyle,
    /// Whether to wrap text to fit bounds width. Default: true.
    wrap: bool,
    /// Cached line count for size_hint (updated during paint).
    cached_line_count: usize,
    /// Cached width used for wrapping.
    cached_wrap_width: f32,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            style: StyleRefinement::default(),
            font_size: theme::font_size::SM,
            color: theme::text::PRIMARY,
            font_style: FontStyle::normal(),
            wrap: true,
            cached_line_count: 1,
            cached_wrap_width: 0.0,
        }
    }

    /// Disable text wrapping (render as single line, may overflow).
    pub fn no_wrap(mut self) -> Self {
        self.wrap = false;
        self
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn bold(mut self) -> Self {
        self.font_style = FontStyle::bold();
        self
    }

    pub fn italic(mut self) -> Self {
        self.font_style = FontStyle::italic();
        self
    }

    pub fn bold_italic(mut self) -> Self {
        self.font_style = FontStyle::bold_italic();
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.font_style = style;
        self
    }

    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Component for Text {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.content.is_empty() {
            return;
        }

        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let color = self.style.text_color.unwrap_or(self.color);
        let line_height = font_size * 1.4;

        if !self.wrap || bounds.size.width <= 0.0 {
            // No wrapping - single line
            let text_run = cx.text.layout_styled(
                &self.content,
                Point::new(bounds.origin.x, bounds.origin.y + font_size),
                font_size,
                color,
                self.font_style,
            );
            cx.scene.draw_text(text_run);
            return;
        }

        // Text wrapping using LineWrapper
        let char_width = font_size * 0.6;
        let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);
        let fragments = [LineFragment::text(&self.content)];
        let boundaries: Vec<_> = wrapper.wrap_line(&fragments, bounds.size.width).collect();

        let mut line_start = 0;
        let mut y = bounds.origin.y + font_size;
        let mut line_count = 0;

        for boundary in &boundaries {
            if y > bounds.origin.y + bounds.size.height {
                break; // Stop if we've exceeded bounds
            }
            let line_text = &self.content[line_start..boundary.ix];
            if !line_text.is_empty() {
                let text_run = cx.text.layout_styled(
                    line_text.trim_end(),
                    Point::new(bounds.origin.x, y),
                    font_size,
                    color,
                    self.font_style,
                );
                cx.scene.draw_text(text_run);
            }
            line_start = boundary.ix;
            y += line_height;
            line_count += 1;
        }

        // Draw remaining text after last boundary
        let remaining = &self.content[line_start..];
        if !remaining.is_empty() && y <= bounds.origin.y + bounds.size.height {
            let text_run = cx.text.layout_styled(
                remaining.trim_end(),
                Point::new(bounds.origin.x, y),
                font_size,
                color,
                self.font_style,
            );
            cx.scene.draw_text(text_run);
            line_count += 1;
        }

        // Update cached values for size_hint
        self.cached_line_count = line_count.max(1);
        self.cached_wrap_width = bounds.size.width;
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let char_count = self.content.chars().count() as f32;
        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let width = char_count * font_size * 0.6;
        let height = font_size * 1.4;
        (Some(width), Some(height))
    }
}

impl Styled for Text {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_new() {
        let text = Text::new("Hello");
        assert_eq!(text.content(), "Hello");
        assert_eq!(text.font_size, theme::font_size::SM);
    }

    #[test]
    fn test_text_builder() {
        let text = Text::new("Test")
            .font_size(18.0)
            .color(theme::text::SECONDARY)
            .bold();

        assert_eq!(text.font_size, 18.0);
        assert!(text.font_style.bold);
        assert!(!text.font_style.italic);
    }

    #[test]
    fn test_text_styles() {
        let bold = Text::new("Bold").bold();
        assert!(bold.font_style.bold);
        assert!(!bold.font_style.italic);

        let italic = Text::new("Italic").italic();
        assert!(!italic.font_style.bold);
        assert!(italic.font_style.italic);

        let bold_italic = Text::new("Both").bold_italic();
        assert!(bold_italic.font_style.bold);
        assert!(bold_italic.font_style.italic);
    }

    #[test]
    fn test_text_set_content() {
        let mut text = Text::new("Original");
        assert_eq!(text.content(), "Original");

        text.set_content("Updated");
        assert_eq!(text.content(), "Updated");
    }

    #[test]
    fn test_text_size_hint() {
        let text = Text::new("Hello");
        let (width, height) = text.size_hint();

        assert!(width.is_some());
        assert!(height.is_some());
        assert!(width.unwrap() > 0.0);
        assert!(height.unwrap() > 0.0);
    }

    #[test]
    fn test_text_empty_size_hint() {
        let text = Text::new("");
        let (width, height) = text.size_hint();

        assert_eq!(width, Some(0.0));
        assert!(height.unwrap() > 0.0);
    }

    #[test]
    fn test_text_component_id() {
        let text = Text::new("Test").with_id(42);
        assert_eq!(Component::id(&text), Some(42));

        let no_id = Text::new("No ID");
        assert_eq!(Component::id(&no_id), None);
    }
}
