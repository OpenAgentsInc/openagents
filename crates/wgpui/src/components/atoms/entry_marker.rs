use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryType {
    #[default]
    User,
    Assistant,
    Tool,
    System,
    Error,
}

impl EntryType {
    fn color(&self) -> Hsla {
        match self {
            EntryType::User => theme::accent::PRIMARY,
            EntryType::Assistant => theme::accent::SECONDARY,
            EntryType::Tool => theme::accent::PURPLE,
            EntryType::System => theme::text::MUTED,
            EntryType::Error => theme::status::ERROR,
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            EntryType::User => ">",
            EntryType::Assistant => "<",
            EntryType::Tool => "$",
            EntryType::System => "*",
            EntryType::Error => "!",
        }
    }
}

pub struct EntryMarker {
    id: Option<ComponentId>,
    entry_type: EntryType,
    size: f32,
    show_icon: bool,
}

impl EntryMarker {
    pub fn new(entry_type: EntryType) -> Self {
        Self {
            id: None,
            entry_type,
            size: 16.0,
            show_icon: true,
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

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn entry_type(&self) -> EntryType {
        self.entry_type
    }

    pub fn set_entry_type(&mut self, entry_type: EntryType) {
        self.entry_type = entry_type;
    }
}

impl Default for EntryMarker {
    fn default() -> Self {
        Self::new(EntryType::default())
    }
}

impl Component for EntryMarker {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let marker_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - self.size) / 2.0,
            bounds.origin.y + (bounds.size.height - self.size) / 2.0,
            self.size,
            self.size,
        );

        cx.scene.draw_quad(
            Quad::new(marker_bounds)
                .with_background(self.entry_type.color().with_alpha(0.2))
                .with_border(self.entry_type.color(), 1.0),
        );

        if self.show_icon {
            let font_size = self.size * 0.6;
            let text_x = marker_bounds.origin.x + (self.size - font_size * 0.6) / 2.0;
            let text_y = marker_bounds.origin.y + self.size * 0.5 - font_size * 0.55;

            let text_run = cx.text.layout(
                self.entry_type.icon(),
                Point::new(text_x, text_y),
                font_size,
                self.entry_type.color(),
            );
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
        (Some(self.size), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_marker_new() {
        let marker = EntryMarker::new(EntryType::User);
        assert_eq!(marker.entry_type(), EntryType::User);
    }

    #[test]
    fn test_entry_type_icons() {
        assert_eq!(EntryType::User.icon(), ">");
        assert_eq!(EntryType::Assistant.icon(), "<");
        assert_eq!(EntryType::Tool.icon(), "$");
    }

    #[test]
    fn test_set_entry_type() {
        let mut marker = EntryMarker::new(EntryType::User);
        marker.set_entry_type(EntryType::Error);
        assert_eq!(marker.entry_type(), EntryType::Error);
    }

    #[test]
    fn test_builder() {
        let marker = EntryMarker::new(EntryType::Tool)
            .with_id(1)
            .size(24.0)
            .show_icon(false);

        assert_eq!(marker.id, Some(1));
        assert_eq!(marker.size, 24.0);
        assert!(!marker.show_icon);
    }
}
