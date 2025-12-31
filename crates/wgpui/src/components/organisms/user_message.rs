use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::MessageHeader;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

pub struct UserMessage {
    id: Option<ComponentId>,
    content: String,
    timestamp: Option<String>,
}

impl UserMessage {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            timestamp: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Default for UserMessage {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for UserMessage {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::MD;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_height = 24.0;
        let mut header = MessageHeader::user();
        if let Some(ts) = &self.timestamp {
            header = header.timestamp(ts.clone());
        }
        header.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + padding,
                bounds.size.width - padding * 2.0,
                header_height,
            ),
            cx,
        );

        let content_y = bounds.origin.y + padding + header_height + theme::spacing::SM;
        let content_height =
            bounds.size.height - padding * 2.0 - header_height - theme::spacing::SM;

        let mut text = Text::new(&self.content)
            .font_size(theme::font_size::BASE)
            .color(theme::text::PRIMARY);
        text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                content_y,
                bounds.size.width - padding * 2.0,
                content_height,
            ),
            cx,
        );
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
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message_new() {
        let msg = UserMessage::new("Hello world");
        assert_eq!(msg.content(), "Hello world");
    }

    #[test]
    fn test_user_message_builder() {
        let msg = UserMessage::new("Test").with_id(1).timestamp("12:30 PM");

        assert_eq!(msg.id, Some(1));
        assert_eq!(msg.timestamp, Some("12:30 PM".to_string()));
    }
}
