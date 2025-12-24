//! Text component for rendering styled text.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, Point, theme};

/// A component that renders text content.
pub struct Text {
    id: Option<ComponentId>,
    content: String,
    font_size: f32,
    color: Hsla,
    style: FontStyle,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            font_size: theme::font_size::SM,
            color: theme::text::PRIMARY,
            style: FontStyle::normal(),
        }
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
        self.style = FontStyle::bold();
        self
    }

    pub fn italic(mut self) -> Self {
        self.style = FontStyle::italic();
        self
    }

    pub fn bold_italic(mut self) -> Self {
        self.style = FontStyle::bold_italic();
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
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

        let text_run = cx.text.layout_styled(
            &self.content,
            Point::new(bounds.origin.x, bounds.origin.y + self.font_size),
            self.font_size,
            self.color,
            self.style,
        );
        cx.scene.draw_text(text_run);
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
        let width = char_count * self.font_size * 0.6;
        let height = self.font_size * 1.4;
        (Some(width), Some(height))
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
        assert!(text.style.bold);
        assert!(!text.style.italic);
    }

    #[test]
    fn test_text_styles() {
        let bold = Text::new("Bold").bold();
        assert!(bold.style.bold);
        assert!(!bold.style.italic);

        let italic = Text::new("Italic").italic();
        assert!(!italic.style.bold);
        assert!(italic.style.italic);

        let bold_italic = Text::new("Both").bold_italic();
        assert!(bold_italic.style.bold);
        assert!(bold_italic.style.italic);
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
