use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentType {
    #[default]
    Text,
    Code,
    Markdown,
    Json,
    Xml,
    Html,
    Image,
    Binary,
    Diff,
    Terminal,
}

impl ContentType {
    fn icon(&self) -> &'static str {
        match self {
            ContentType::Text => "T",
            ContentType::Code => "<>",
            ContentType::Markdown => "M",
            ContentType::Json => "{}",
            ContentType::Xml => "</>",
            ContentType::Html => "H",
            ContentType::Image => "I",
            ContentType::Binary => "B",
            ContentType::Diff => "±",
            ContentType::Terminal => "$",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            ContentType::Text => theme::text::MUTED,
            ContentType::Code => theme::accent::PRIMARY,
            ContentType::Markdown => theme::accent::SECONDARY,
            ContentType::Json => theme::status::WARNING,
            ContentType::Xml | ContentType::Html => theme::accent::PURPLE,
            ContentType::Image => theme::status::SUCCESS,
            ContentType::Binary => theme::text::MUTED,
            ContentType::Diff => theme::accent::PRIMARY,
            ContentType::Terminal => theme::accent::SECONDARY,
        }
    }
}

pub struct ContentTypeIcon {
    id: Option<ComponentId>,
    content_type: ContentType,
    size: f32,
    show_background: bool,
}

impl ContentTypeIcon {
    pub fn new(content_type: ContentType) -> Self {
        Self {
            id: None,
            content_type,
            size: 20.0,
            show_background: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn show_background(mut self, show: bool) -> Self {
        self.show_background = show;
        self
    }

    pub fn content_type(&self) -> ContentType {
        self.content_type
    }

    pub fn set_content_type(&mut self, content_type: ContentType) {
        self.content_type = content_type;
    }
}

impl Default for ContentTypeIcon {
    fn default() -> Self {
        Self::new(ContentType::default())
    }
}

impl Component for ContentTypeIcon {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let icon_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - self.size) / 2.0,
            bounds.origin.y + (bounds.size.height - self.size) / 2.0,
            self.size,
            self.size,
        );

        if self.show_background {
            cx.scene.draw_quad(
                Quad::new(icon_bounds)
                    .with_background(theme::bg::MUTED)
                    .with_border(self.content_type.color(), 1.0),
            );
        }

        let icon = self.content_type.icon();
        let font_size = if icon.len() > 1 {
            self.size * 0.4
        } else {
            self.size * 0.6
        };
        let text_width = icon.len() as f32 * font_size * 0.6;
        let text_x = icon_bounds.origin.x + (self.size - text_width) / 2.0;
        let text_y = icon_bounds.origin.y + self.size * 0.5 - font_size * 0.55;

        let text_run = cx.text.layout(
            icon,
            Point::new(text_x, text_y),
            font_size,
            self.content_type.color(),
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
        (Some(self.size), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_icon_new() {
        let icon = ContentTypeIcon::new(ContentType::Code);
        assert_eq!(icon.content_type(), ContentType::Code);
    }

    #[test]
    fn test_content_type_icons() {
        assert_eq!(ContentType::Code.icon(), "<>");
        assert_eq!(ContentType::Json.icon(), "{}");
        assert_eq!(ContentType::Terminal.icon(), "$");
    }

    #[test]
    fn test_set_content_type() {
        let mut icon = ContentTypeIcon::new(ContentType::Text);
        icon.set_content_type(ContentType::Diff);
        assert_eq!(icon.content_type(), ContentType::Diff);
    }

    #[test]
    fn test_builder() {
        let icon = ContentTypeIcon::new(ContentType::Markdown)
            .with_id(1)
            .size(24.0)
            .show_background(false);

        assert_eq!(icon.id, Some(1));
        assert_eq!(icon.size, 24.0);
        assert!(!icon.show_background);
    }
}
