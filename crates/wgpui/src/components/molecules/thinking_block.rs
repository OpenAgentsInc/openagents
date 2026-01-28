use crate::components::context::{EventContext, PaintContext};
use crate::components::organisms::MarkdownView;
use crate::components::{Component, ComponentId, EventResult};
use crate::input::MouseButton;
use crate::markdown::{MarkdownConfig, MarkdownParser};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct ThinkingBlock {
    id: Option<ComponentId>,
    content: String,
    expanded: bool,
    hovered: bool,
    header_label: String,
    markdown: MarkdownView,
    markdown_config: MarkdownConfig,
    rendered_text: String,
}

impl ThinkingBlock {
    pub fn new(content: impl Into<String>) -> Self {
        let raw = content.into();
        let content = strip_task_markers(&raw);
        let mut markdown_config = MarkdownConfig::default();
        markdown_config.base_font_size = theme::font_size::XS;
        markdown_config.header_sizes = [1.0; 6];
        let header_label = extract_header_label(&content);
        let document = MarkdownParser::with_config(markdown_config.clone()).parse(&content);
        let markdown = MarkdownView::new(document)
            .with_config(markdown_config.clone())
            .show_copy_button(false)
            .copy_button_on_hover(false);
        Self {
            id: None,
            content,
            expanded: false,
            hovered: false,
            header_label,
            markdown,
            markdown_config,
            rendered_text: String::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
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
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Default for ThinkingBlock {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for ThinkingBlock {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::XS;
        let header_height = 20.0;
        let icon_size = 10.0;
        let icon_radius = 2.0;
        let icon_y = bounds.origin.y + (header_height - icon_size) * 0.5;
        let icon_bounds = Bounds::new(
            bounds.origin.x + padding,
            icon_y,
            icon_size,
            icon_size,
        );
        cx.scene.draw_quad(
            Quad::new(icon_bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(icon_radius),
        );

        let label_x = icon_bounds.origin.x + icon_size + theme::spacing::XS;
        let label_y = bounds.origin.y + header_height * 0.5 - theme::font_size::XS * 0.55;
        let label_run = cx.text.layout_mono(
            &self.header_label,
            Point::new(label_x, label_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(label_run);

        if self.hovered || self.expanded {
            let chevron = if self.expanded { "v" } else { ">" };
            let chevron_width = theme::font_size::XS * 0.6;
            let chevron_x = bounds.origin.x + bounds.size.width - padding - chevron_width;
            let chevron_y = label_y;
            let chevron_run = cx.text.layout_mono(
                chevron,
                Point::new(chevron_x, chevron_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(chevron_run);
        }

        if !self.expanded {
            return;
        }

        let content_y = bounds.origin.y + header_height + theme::spacing::XS;
        let content_indent = label_x;
        let content_bounds = Bounds::new(
            content_indent,
            content_y,
            bounds.size.width - (content_indent - bounds.origin.x) - padding,
            (bounds.size.height - (content_y - bounds.origin.y) - padding).max(0.0),
        );

        let border_x = content_indent - theme::spacing::XS;
        let border_bounds = Bounds::new(
            border_x,
            content_y,
            1.0,
            (bounds.size.height - (content_y - bounds.origin.y) - padding).max(0.0),
        );
        cx.scene
            .draw_quad(Quad::new(border_bounds).with_background(theme::border::DEFAULT));

        if self.content != self.rendered_text {
            let document =
                MarkdownParser::with_config(self.markdown_config.clone()).parse(&self.content);
            self.markdown.set_document(document);
            self.header_label = extract_header_label(&self.content);
            self.rendered_text.clear();
            self.rendered_text.push_str(&self.content);
        }

        self.markdown.paint(content_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let header_bounds =
                        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 20.0);
                    if header_bounds.contains(Point::new(*x, *y)) {
                        self.toggle();
                        return EventResult::Handled;
                    }
                }
            }
            _ => {}
        }

        if self.expanded {
            let content_indent = bounds.origin.x + theme::spacing::XS + 10.0 + theme::spacing::XS;
            let content_y = bounds.origin.y + 20.0 + theme::spacing::XS;
            let content_bounds = Bounds::new(
                content_indent,
                content_y,
                bounds.size.width - (content_indent - bounds.origin.x) - theme::spacing::XS,
                (bounds.size.height - (content_y - bounds.origin.y) - theme::spacing::XS).max(0.0),
            );
            return self.markdown.event(event, content_bounds, cx);
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let header_height = 20.0;
        if self.expanded {
            let content_height = self.markdown.size_hint().1.unwrap_or(0.0);
            (
                None,
                Some(header_height + theme::spacing::XS + content_height + theme::spacing::XS),
            )
        } else {
            (None, Some(header_height))
        }
    }
}

fn extract_header_label(content: &str) -> String {
    for line in content.lines() {
        let mut label = line.trim();
        if label.is_empty() {
            continue;
        }
        label = label.trim_start_matches('>');
        label = label.trim();
        if let Some(stripped) = label.strip_prefix("- ") {
            label = stripped.trim();
        }
        let label = label.trim_matches(|c| c == '*' || c == '_' || c == '`').trim();
        if !label.is_empty() {
            return label.to_string();
        }
    }
    "Thinking".to_string()
}

fn strip_task_markers(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for (idx, line) in content.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let indent_len = line.len().saturating_sub(trimmed.len());
        out.push_str(&line[..indent_len]);

        let rest = trimmed;
        let mut cleaned = None;
        for prefix in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "* [X] ", "+ [ ] ", "+ [x] ", "+ [X] "] {
            if rest.starts_with(prefix) {
                let bullet = &prefix[..2];
                cleaned = Some(format!("{bullet}{}", &rest[prefix.len()..]));
                break;
            }
        }
        if cleaned.is_none() {
            for prefix in ["[ ] ", "[x] ", "[X] "] {
                if rest.starts_with(prefix) {
                    cleaned = Some(rest[prefix.len()..].to_string());
                    break;
                }
            }
        }
        out.push_str(cleaned.as_deref().unwrap_or(rest));
    }
    out
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
        let block = ThinkingBlock::new("Content").with_id(1).expanded(true);

        assert_eq!(block.id, Some(1));
        assert!(block.is_expanded());
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
