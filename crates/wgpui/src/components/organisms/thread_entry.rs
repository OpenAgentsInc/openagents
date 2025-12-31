use crate::clipboard::copy_to_clipboard;
use crate::components::atoms::EntryType as AtomEntryType;
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::{EntryAction, EntryActions};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryType {
    #[default]
    User,
    Assistant,
    Tool,
    System,
    Error,
}

impl From<EntryType> for AtomEntryType {
    fn from(t: EntryType) -> Self {
        match t {
            EntryType::User => AtomEntryType::User,
            EntryType::Assistant => AtomEntryType::Assistant,
            EntryType::Tool => AtomEntryType::Tool,
            EntryType::System => AtomEntryType::System,
            EntryType::Error => AtomEntryType::Error,
        }
    }
}

pub struct ThreadEntry {
    id: Option<ComponentId>,
    entry_type: EntryType,
    content: Box<dyn Component>,
    show_actions: bool,
    hovered: bool,
    on_copy: Option<Box<dyn FnMut()>>,
    on_retry: Option<Box<dyn FnMut()>>,
    /// Text that will be copied to clipboard when Copy is clicked.
    copyable_text: Option<String>,
    /// Persistent actions component (not recreated each frame).
    actions: EntryActions,
}

impl ThreadEntry {
    pub fn new(entry_type: EntryType, content: impl Component + 'static) -> Self {
        Self {
            id: None,
            entry_type,
            content: Box::new(content),
            show_actions: false,
            hovered: false,
            on_copy: None,
            on_retry: None,
            copyable_text: None,
            actions: EntryActions::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_actions(mut self, show: bool) -> Self {
        self.show_actions = show;
        self
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    pub fn on_retry<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_retry = Some(Box::new(f));
        self
    }

    /// Set the text that will be copied to clipboard when Copy is clicked.
    pub fn copyable_text(mut self, text: impl Into<String>) -> Self {
        self.copyable_text = Some(text.into());
        self
    }

    pub fn entry_type(&self) -> EntryType {
        self.entry_type
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Replace the content component (useful for updating tool status)
    pub fn set_content(&mut self, content: impl Component + 'static) {
        self.content = Box::new(content);
    }
}

impl Component for ThreadEntry {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Dense but readable padding (4px)
        let padding = 4.0;

        if self.hovered {
            cx.scene
                .draw_quad(Quad::new(bounds).with_background(theme::bg::MUTED.with_alpha(0.3)));
        }

        let mut content_bounds = bounds;
        content_bounds.origin.x += padding;
        content_bounds.origin.y += padding;
        content_bounds.size.width -= padding * 2.0;
        content_bounds.size.height -= padding * 2.0;

        // No reserved space for actions - they overlay on hover
        self.content.paint(content_bounds, cx);

        // Actions overlay in top-right corner when hovered
        if self.show_actions || self.hovered {
            let actions_bounds = Bounds::new(
                bounds.origin.x + bounds.size.width - padding - 100.0,
                bounds.origin.y + padding,
                100.0,
                20.0,
            );
            self.actions.paint(actions_bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if let InputEvent::MouseMove { x, y } = event {
            let was_hovered = self.hovered;
            self.hovered = bounds.contains(Point::new(*x, *y));
            if was_hovered != self.hovered {
                return EventResult::Handled;
            }
        }

        // Dense but readable padding (4px)
        let padding = 4.0;
        let mut content_bounds = bounds;
        content_bounds.origin.x += padding;
        content_bounds.origin.y += padding;
        content_bounds.size.width -= padding * 2.0;
        content_bounds.size.height -= padding * 2.0;

        // Actions overlay in top-right corner
        let actions_bounds = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 100.0,
            bounds.origin.y + padding,
            100.0,
            20.0,
        );

        // Handle actions events when hovered or shown
        if self.show_actions || self.hovered {
            if let EventResult::Handled = self.actions.event(event, actions_bounds, cx) {
                // Check if an action was triggered
                if let Some(action) = self.actions.take_triggered_action() {
                    match action {
                        EntryAction::Copy => {
                            if let Some(ref text) = self.copyable_text {
                                let _ = copy_to_clipboard(text);
                            }
                        }
                        _ => {
                            // Other actions not yet implemented
                        }
                    }
                }
                return EventResult::Handled;
            }
        }

        self.content.event(event, content_bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let (w, h) = self.content.size_hint();
        // Dense but readable padding (4px on each side = 8px total)
        let padding = 8.0;
        // No reserved space for actions - they overlay on hover
        (w.map(|w| w + padding), h.map(|h| h + padding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Text;

    #[test]
    fn test_thread_entry_new() {
        let entry = ThreadEntry::new(EntryType::User, Text::new("Hello"));
        assert_eq!(entry.entry_type(), EntryType::User);
        assert!(!entry.is_hovered());
    }

    #[test]
    fn test_thread_entry_builder() {
        let entry = ThreadEntry::new(EntryType::Assistant, Text::new("Response"))
            .with_id(1)
            .show_actions(true);

        assert_eq!(entry.id, Some(1));
        assert!(entry.show_actions);
    }

    #[test]
    fn test_entry_type_conversion() {
        assert_eq!(AtomEntryType::from(EntryType::User), AtomEntryType::User);
        assert_eq!(AtomEntryType::from(EntryType::Tool), AtomEntryType::Tool);
    }
}
