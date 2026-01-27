use crate::components::atoms::{EntryMarker, EntryType, Model, ModelBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, theme};

pub struct MessageHeader {
    id: Option<ComponentId>,
    entry_type: EntryType,
    model: Option<Model>,
    timestamp: Option<String>,
    author: Option<String>,
}

impl MessageHeader {
    pub fn new(entry_type: EntryType) -> Self {
        Self {
            id: None,
            entry_type,
            model: None,
            timestamp: None,
            author: None,
        }
    }

    pub fn user() -> Self {
        Self::new(EntryType::User)
    }

    pub fn assistant(model: Model) -> Self {
        Self::new(EntryType::Assistant).model(model)
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn model(mut self, model: Model) -> Self {
        self.model = Some(model);
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn entry_type(&self) -> EntryType {
        self.entry_type
    }
}

impl Default for MessageHeader {
    fn default() -> Self {
        Self::new(EntryType::User)
    }
}

impl Component for MessageHeader {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut x = bounds.origin.x;
        let marker_size = 24.0;

        let mut marker = EntryMarker::new(self.entry_type).size(marker_size);
        marker.paint(
            Bounds::new(x, bounds.origin.y, marker_size, bounds.size.height),
            cx,
        );
        x += marker_size + theme::spacing::SM;

        if let Some(author) = &self.author {
            let font_size = theme::font_size::BASE;
            let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;
            let text_run = cx.text.layout_mono(
                author,
                Point::new(x, text_y),
                font_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(text_run);
            x += author.len() as f32 * font_size * 0.6 + theme::spacing::SM;
        }

        if let Some(model) = self.model {
            let mut badge = ModelBadge::new(model);
            let (badge_w, badge_h) = badge.size_hint();
            badge.paint(
                Bounds::new(
                    x,
                    bounds.origin.y,
                    badge_w.unwrap_or(60.0),
                    badge_h.unwrap_or(bounds.size.height),
                ),
                cx,
            );
            x += badge_w.unwrap_or(60.0) + theme::spacing::SM;
        }

        if let Some(ts) = &self.timestamp {
            let font_size = theme::font_size::SM;
            let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;
            let text_run = cx
                .text
                .layout(ts, Point::new(x, text_y), font_size, theme::text::MUTED);
            cx.scene.draw_text(text_run);
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
        (None, Some(28.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header_user() {
        let header = MessageHeader::user();
        assert_eq!(header.entry_type(), EntryType::User);
    }

    #[test]
    fn test_message_header_assistant() {
        let header = MessageHeader::assistant(Model::CodexSonnet);
        assert_eq!(header.entry_type(), EntryType::Assistant);
        assert_eq!(header.model, Some(Model::CodexSonnet));
    }

    #[test]
    fn test_message_header_builder() {
        let header = MessageHeader::user()
            .with_id(1)
            .author("User")
            .timestamp("12:34");

        assert_eq!(header.id, Some(1));
        assert_eq!(header.author, Some("User".to_string()));
        assert_eq!(header.timestamp, Some("12:34".to_string()));
    }
}
