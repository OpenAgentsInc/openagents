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
    /// Default wrap width estimate for size_hint before paint.
    /// Use conservative estimate (narrower) to avoid underestimating height.
    const DEFAULT_WRAP_WIDTH: f32 = 400.0;

    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        // Pre-calculate estimated line count for better size_hint
        let font_size = theme::font_size::SM;
        let char_width = font_size * 0.6;
        let chars_per_line = (Self::DEFAULT_WRAP_WIDTH / char_width) as usize;

        // Count explicit newlines + estimate wrapped lines for each paragraph
        // Use ceiling division to ensure we have enough height
        let mut estimated_lines = 0;
        for line in content.split('\n') {
            let char_count = line.chars().count();
            let line_wrap_count = if chars_per_line > 0 && char_count > 0 {
                // Ceiling division: (a + b - 1) / b
                ((char_count + chars_per_line - 1) / chars_per_line).max(1)
            } else {
                1
            };
            estimated_lines += line_wrap_count;
        }
        estimated_lines = estimated_lines.max(1);

        Self {
            id: None,
            content,
            style: StyleRefinement::default(),
            font_size,
            color: theme::text::PRIMARY,
            font_style: FontStyle::normal(),
            wrap: true,
            cached_line_count: estimated_lines,
            cached_wrap_width: Self::DEFAULT_WRAP_WIDTH,
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

    /// Calculate size hint with a specific wrap width.
    /// Call this before painting when you know the available width.
    pub fn size_hint_with_width(&mut self, wrap_width: f32) -> (Option<f32>, Option<f32>) {
        if !self.wrap || wrap_width <= 0.0 {
            return Component::size_hint(self);
        }

        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let line_height = font_size * 1.4;
        let char_width = font_size * 0.6;

        // Calculate wrapped line count accounting for newlines
        let mut total_lines = 0;

        for paragraph in self.content.split('\n') {
            if paragraph.is_empty() {
                total_lines += 1;
                continue;
            }

            let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);
            let fragments = [LineFragment::text(paragraph)];
            let boundaries: Vec<_> = wrapper.wrap_line(&fragments, wrap_width).collect();

            // boundaries.len() gives wrap points, add 1 for the final segment
            total_lines += boundaries.len().max(1);
        }
        total_lines = total_lines.max(1);

        // Update cache for size_hint()
        self.cached_line_count = total_lines;
        self.cached_wrap_width = wrap_width;

        let height = line_height * total_lines as f32;
        (None, Some(height))
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
        let char_width = font_size * 0.6;

        let mut y = bounds.origin.y + font_size;
        let mut total_line_count = 0;

        // Split by newlines first, then wrap each paragraph
        for paragraph in self.content.split('\n') {
            if y > bounds.origin.y + bounds.size.height {
                break;
            }

            if paragraph.is_empty() {
                // Empty line - just advance y
                y += line_height;
                total_line_count += 1;
                continue;
            }

            if !self.wrap || bounds.size.width <= 0.0 {
                // No wrapping - render paragraph as single line
                let text_run = cx.text.layout_styled(
                    paragraph,
                    Point::new(bounds.origin.x, y),
                    font_size,
                    color,
                    self.font_style,
                );
                cx.scene.draw_text(text_run);
                y += line_height;
                total_line_count += 1;
            } else {
                // Wrap this paragraph
                let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);
                let fragments = [LineFragment::text(paragraph)];
                let boundaries: Vec<_> = wrapper.wrap_line(&fragments, bounds.size.width).collect();

                let mut line_start = 0;

                for boundary in &boundaries {
                    if y > bounds.origin.y + bounds.size.height {
                        break;
                    }
                    let line_text = &paragraph[line_start..boundary.ix];
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
                    total_line_count += 1;
                }

                // Draw remaining text after last boundary
                let remaining = &paragraph[line_start..];
                if !remaining.is_empty() && y <= bounds.origin.y + bounds.size.height {
                    let text_run = cx.text.layout_styled(
                        remaining.trim_end(),
                        Point::new(bounds.origin.x, y),
                        font_size,
                        color,
                        self.font_style,
                    );
                    cx.scene.draw_text(text_run);
                    y += line_height;
                    total_line_count += 1;
                }
            }
        }

        // Update cached values for size_hint
        self.cached_line_count = total_line_count.max(1);
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
        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let line_height = font_size * 1.4;
        let char_width = font_size * 0.6;

        // Use cached line count if we have it from a previous paint
        if self.cached_line_count > 0 && self.cached_wrap_width > 0.0 {
            let height = line_height * self.cached_line_count as f32;
            let max_line_width = self
                .content
                .split('\n')
                .map(|line| line.chars().count() as f32 * char_width)
                .fold(0.0f32, f32::max);
            return (Some(max_line_width), Some(height));
        }

        // Estimate line count accounting for newlines and wrapping
        let wrap_width = self.cached_wrap_width.max(Self::DEFAULT_WRAP_WIDTH);
        let chars_per_line = (wrap_width / char_width).max(1.0) as usize;

        let mut estimated_lines = 0;
        let mut max_line_width = 0.0f32;

        for paragraph in self.content.split('\n') {
            let char_count = paragraph.chars().count();
            let paragraph_width = char_count as f32 * char_width;
            max_line_width = max_line_width.max(paragraph_width);

            if self.wrap && chars_per_line > 0 && char_count > 0 {
                // Ceiling division to ensure enough height
                estimated_lines += ((char_count + chars_per_line - 1) / chars_per_line).max(1);
            } else {
                estimated_lines += 1;
            }
        }
        estimated_lines = estimated_lines.max(1);

        let height = line_height * estimated_lines as f32;
        (Some(max_line_width), Some(height))
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
