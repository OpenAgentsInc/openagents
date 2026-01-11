use crate::components::atoms::{Model, StreamingIndicator};
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::MessageHeader;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, InputEvent, Quad, theme};

pub struct AssistantMessage {
    id: Option<ComponentId>,
    content: String,
    model: Model,
    timestamp: Option<String>,
    streaming: bool,
    streaming_indicator: StreamingIndicator,
}

impl AssistantMessage {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            model: Model::CodexSonnet,
            timestamp: None,
            streaming: false,
            streaming_indicator: StreamingIndicator::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    pub fn append_content(&mut self, text: &str) {
        self.content.push_str(text);
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    pub fn tick(&mut self) {
        if self.streaming {
            self.streaming_indicator.tick();
        }
    }
}

impl Default for AssistantMessage {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for AssistantMessage {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::MD;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_height = 24.0;
        let mut header = MessageHeader::assistant(self.model);
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
        let mut content_height =
            bounds.size.height - padding * 2.0 - header_height - theme::spacing::SM;

        if self.streaming {
            content_height -= 20.0;
        }

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

        if self.streaming {
            self.streaming_indicator.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    bounds.origin.y + bounds.size.height - padding - 16.0,
                    100.0,
                    16.0,
                ),
                cx,
            );
        }
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
    fn test_assistant_message_new() {
        let msg = AssistantMessage::new("Hello from Codex");
        assert_eq!(msg.content(), "Hello from Codex");
        assert!(!msg.is_streaming());
    }

    #[test]
    fn test_assistant_message_builder() {
        let msg = AssistantMessage::new("Response")
            .with_id(1)
            .model(Model::CodexOpus)
            .streaming(true)
            .timestamp("12:30 PM");

        assert_eq!(msg.id, Some(1));
        assert_eq!(msg.model, Model::CodexOpus);
        assert!(msg.is_streaming());
    }

    #[test]
    fn test_append_content() {
        let mut msg = AssistantMessage::new("Hello");
        msg.append_content(" world");
        assert_eq!(msg.content(), "Hello world");
    }
}
