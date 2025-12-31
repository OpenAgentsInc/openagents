use crate::components::atoms::ThinkingToggle;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct ThinkingBlock {
    id: Option<ComponentId>,
    content: String,
    expanded: bool,
    toggle: ThinkingToggle,
    max_collapsed_lines: usize,
}

impl ThinkingBlock {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            expanded: false,
            toggle: ThinkingToggle::new(),
            max_collapsed_lines: 3,
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
        let font_size = theme::font_size::SM;
        let line_height = font_size * 1.4;

        let visible = self.visible_content();
        for (i, line) in visible.lines().enumerate() {
            let y = content_y + i as f32 * line_height;
            if y + line_height > bounds.origin.y + bounds.size.height {
                break;
            }
            let text_run = cx.text.layout(
                line,
                Point::new(bounds.origin.x + padding, y),
                font_size,
                theme::text::MUTED,
            );
            cx.scene.draw_text(text_run);
        }

        if !self.expanded && self.content.lines().count() > self.max_collapsed_lines {
            let ellipsis_y = content_y + self.max_collapsed_lines as f32 * line_height;
            let text_run = cx.text.layout(
                "...",
                Point::new(bounds.origin.x + padding, ellipsis_y),
                font_size,
                theme::text::MUTED,
            );
            cx.scene.draw_text(text_run);
        }
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
        }
        result
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
