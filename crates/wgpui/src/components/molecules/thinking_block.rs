use crate::components::atoms::ThinkingToggle;
use crate::components::context::{EventContext, PaintContext};
use crate::components::organisms::MarkdownView;
use crate::components::{Component, ComponentId, EventResult};
use crate::markdown::MarkdownParser;
use crate::{Bounds, InputEvent, Quad, theme};

pub struct ThinkingBlock {
    id: Option<ComponentId>,
    content: String,
    expanded: bool,
    toggle: ThinkingToggle,
    max_collapsed_lines: usize,
    markdown: MarkdownView,
    rendered_text: String,
}

impl ThinkingBlock {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let document = MarkdownParser::new().parse(&content);
        let markdown = MarkdownView::new(document)
            .show_copy_button(false)
            .copy_button_on_hover(false);
        Self {
            id: None,
            content,
            expanded: false,
            toggle: ThinkingToggle::new(),
            max_collapsed_lines: 3,
            markdown,
            rendered_text: String::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self.toggle = self.toggle.expanded(expanded);
        self
    }

    pub fn max_collapsed_lines(mut self, lines: usize) -> Self {
        self.max_collapsed_lines = lines;
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.toggle.set_expanded(expanded);
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        self.toggle.set_expanded(self.expanded);
    }

    fn visible_content(&self) -> &str {
        if self.expanded {
            &self.content
        } else {
            let lines: Vec<&str> = self
                .content
                .lines()
                .take(self.max_collapsed_lines)
                .collect();
            if lines.len() < self.content.lines().count() {
                return &self.content[..self
                    .content
                    .lines()
                    .take(self.max_collapsed_lines)
                    .map(|l| l.len() + 1)
                    .sum::<usize>()
                    .saturating_sub(1)];
            }
            &self.content
        }
    }
}

impl Default for ThinkingBlock {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for ThinkingBlock {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        let toggle_height = 24.0;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::MUTED.with_alpha(0.5))
                .with_border(theme::border::SUBTLE, 1.0),
        );

        self.toggle.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                toggle_height,
            ),
            cx,
        );

        let content_y = bounds.origin.y + padding + toggle_height + theme::spacing::XS;
        let content_bounds = Bounds::new(
            bounds.origin.x + padding,
            content_y,
            bounds.size.width - padding * 2.0,
            (bounds.size.height - (content_y - bounds.origin.y) - padding).max(0.0),
        );

        let visible = self.visible_content().to_string();
        if visible != self.rendered_text {
            let document = MarkdownParser::new().parse(&visible);
            self.markdown.set_document(document);
            self.rendered_text.clear();
            self.rendered_text.push_str(&visible);
        }

        self.markdown.paint(content_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::SM;
        let toggle_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            24.0,
        );

        let result = self.toggle.event(event, toggle_bounds, cx);
        if result == EventResult::Handled {
            self.expanded = self.toggle.is_expanded();
            self.rendered_text.clear();
        }
        let markdown_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding + 24.0 + theme::spacing::XS,
            bounds.size.width - padding * 2.0,
            (bounds.size.height - (padding + 24.0 + theme::spacing::XS + padding)).max(0.0),
        );
        let markdown_handled =
            matches!(self.markdown.event(event, markdown_bounds, cx), EventResult::Handled);
        if markdown_handled {
            EventResult::Handled
        } else {
            result
        }
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let padding = theme::spacing::SM;
        let toggle_height = 24.0;
        let font_size = theme::font_size::SM;
        let line_height = font_size * 1.4;

        let visible_lines = if self.expanded {
            self.content.lines().count()
        } else {
            self.max_collapsed_lines.min(self.content.lines().count()) + 1
        };

        let height =
            padding * 2.0 + toggle_height + theme::spacing::XS + visible_lines as f32 * line_height;
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_block_new() {
        let block = ThinkingBlock::new("Some thinking content");
        assert_eq!(block.content(), "Some thinking content");
        assert!(!block.is_expanded());
    }

    #[test]
    fn test_thinking_block_builder() {
        let block = ThinkingBlock::new("Content")
            .with_id(1)
            .expanded(true)
            .max_collapsed_lines(5);

        assert_eq!(block.id, Some(1));
        assert!(block.is_expanded());
        assert_eq!(block.max_collapsed_lines, 5);
    }

    #[test]
    fn test_toggle() {
        let mut block = ThinkingBlock::new("Content");
        assert!(!block.is_expanded());
        block.toggle();
        assert!(block.is_expanded());
        block.toggle();
        assert!(!block.is_expanded());
    }
}
