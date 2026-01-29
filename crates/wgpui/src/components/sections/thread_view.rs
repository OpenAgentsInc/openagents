use crate::components::context::{EventContext, PaintContext};
use crate::components::organisms::ThreadEntry;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct ThreadView {
    id: Option<ComponentId>,
    entries: Vec<ThreadEntry>,
    scroll_offset: f32,
    content_height: f32,
    auto_scroll: bool,
    item_spacing: f32,
    on_entry_click: Option<Box<dyn FnMut(usize)>>,
    lock_to_bottom: bool,
}

impl ThreadView {
    pub fn new() -> Self {
        Self {
            id: None,
            entries: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            auto_scroll: true,
            item_spacing: 6.0, // Dense but readable spacing between entries
            on_entry_click: None,
            lock_to_bottom: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn entries(mut self, entries: Vec<ThreadEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn auto_scroll(mut self, auto: bool) -> Self {
        self.auto_scroll = auto;
        self
    }

    pub fn item_spacing(mut self, spacing: f32) -> Self {
        self.item_spacing = spacing;
        self
    }

    pub fn on_entry_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(usize) + 'static,
    {
        self.on_entry_click = Some(Box::new(f));
        self
    }

    pub fn push_entry(&mut self, entry: ThreadEntry) {
        // Only auto-scroll if already at bottom (within threshold)
        let was_at_bottom = self.is_at_bottom();
        self.entries.push(entry);
        if self.auto_scroll && was_at_bottom {
            self.lock_to_bottom = true;
            self.scroll_to_bottom();
        }
    }

    /// Check if scroll position is at or near the bottom
    fn is_at_bottom(&self) -> bool {
        // Consider "at bottom" if within 50px of the end
        self.scroll_offset >= (self.content_height - 50.0).max(0.0)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
        self.lock_to_bottom = true;
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove and return the last entry (useful for updating tool status)
    pub fn pop_entry(&mut self) -> Option<ThreadEntry> {
        self.entries.pop()
    }

    /// Get mutable reference to last entry
    pub fn last_entry_mut(&mut self) -> Option<&mut ThreadEntry> {
        self.entries.last_mut()
    }

    /// Get mutable reference to entry at index
    pub fn entry_mut(&mut self, index: usize) -> Option<&mut ThreadEntry> {
        self.entries.get_mut(index)
    }

    pub fn is_action_hovered(&self) -> bool {
        self.entries.iter().any(|entry| entry.is_action_hovered())
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.content_height;
        self.lock_to_bottom = true;
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0.0;
        self.lock_to_bottom = false;
    }

    fn calculate_content_height(&self) -> f32 {
        let mut height = 0.0;
        for entry in &self.entries {
            let (_, h) = entry.size_hint();
            height += h.unwrap_or(30.0) + self.item_spacing;
        }
        height
    }
}

impl Default for ThreadView {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ThreadView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // No background - let parent control transparency

        self.content_height = self.calculate_content_height();

        let max_scroll = (self.content_height - bounds.size.height).max(0.0);
        if self.auto_scroll && self.lock_to_bottom {
            let target = max_scroll;
            let delta = target - self.scroll_offset;
            if delta.abs() > bounds.size.height * 0.5 {
                self.scroll_offset = target;
            } else if delta.abs() > 0.5 {
                self.scroll_offset += delta * 0.45;
            } else {
                self.scroll_offset = target;
            }
        }
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene.push_clip(bounds);

        let mut y = bounds.origin.y - self.scroll_offset;

        for entry in &mut self.entries {
            let (_, entry_height) = entry.size_hint();
            let height = entry_height.unwrap_or(30.0);

            if y + height >= bounds.origin.y && y <= bounds.origin.y + bounds.size.height {
                let entry_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, height);
                entry.paint(entry_bounds, cx);
            }

            y += height + self.item_spacing;
        }

        cx.scene.pop_clip();

        if self.content_height > bounds.size.height {
            let scrollbar_width = 6.0;
            let scrollbar_height = bounds.size.height * (bounds.size.height / self.content_height);
            let scrollbar_y =
                bounds.origin.y + (self.scroll_offset / self.content_height) * bounds.size.height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + bounds.size.width - scrollbar_width - 2.0,
                    scrollbar_y,
                    scrollbar_width,
                    scrollbar_height,
                ))
                .with_background(theme::bg::MUTED),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::Scroll { dy, .. } => {
                let max_scroll = (self.content_height - bounds.size.height).max(0.0);
                // Positive dy = scroll wheel down = show content below = increase offset
                self.scroll_offset = (self.scroll_offset + dy).clamp(0.0, max_scroll);
                self.lock_to_bottom = self.is_at_bottom();
                return EventResult::Handled;
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if bounds.contains(point) {
                    let mut check_y = bounds.origin.y - self.scroll_offset;
                    for (i, entry) in self.entries.iter().enumerate() {
                        let (_, entry_height) = entry.size_hint();
                        let height = entry_height.unwrap_or(30.0);
                        let entry_bounds =
                            Bounds::new(bounds.origin.x, check_y, bounds.size.width, height);

                        if entry_bounds.contains(point) {
                            if let Some(callback) = &mut self.on_entry_click {
                                callback(i);
                            }
                            return EventResult::Handled;
                        }

                        check_y += height + self.item_spacing;
                    }
                }
            }
            _ => {}
        }

        let mut check_y = bounds.origin.y - self.scroll_offset;
        for entry in &mut self.entries {
            let (_, entry_height) = entry.size_hint();
            let height = entry_height.unwrap_or(30.0);
            let entry_bounds = Bounds::new(bounds.origin.x, check_y, bounds.size.width, height);

            let result = entry.event(event, entry_bounds, cx);
            if result == EventResult::Handled {
                return result;
            }

            check_y += height + self.item_spacing;
        }

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
    use crate::components::Text;
    use crate::components::organisms::ThreadEntryType as EntryType;

    #[test]
    fn test_thread_view_new() {
        let view = ThreadView::new();
        assert_eq!(view.entry_count(), 0);
        assert!(view.auto_scroll);
    }

    #[test]
    fn test_thread_view_push_entry() {
        let mut view = ThreadView::new();
        view.push_entry(ThreadEntry::new(EntryType::User, Text::new("Hello")));
        view.push_entry(ThreadEntry::new(EntryType::Assistant, Text::new("Hi")));

        assert_eq!(view.entry_count(), 2);
    }

    #[test]
    fn test_thread_view_clear() {
        let mut view = ThreadView::new();
        view.push_entry(ThreadEntry::new(EntryType::User, Text::new("Test")));
        assert_eq!(view.entry_count(), 1);

        view.clear();
        assert_eq!(view.entry_count(), 0);
    }

    #[test]
    fn test_thread_view_builder() {
        let entries = vec![
            ThreadEntry::new(EntryType::User, Text::new("One")),
            ThreadEntry::new(EntryType::Assistant, Text::new("Two")),
        ];

        let view = ThreadView::new()
            .with_id(1)
            .entries(entries)
            .auto_scroll(false)
            .item_spacing(16.0);

        assert_eq!(view.id, Some(1));
        assert_eq!(view.entry_count(), 2);
        assert!(!view.auto_scroll);
        assert_eq!(view.item_spacing, 16.0);
    }

    #[test]
    fn test_thread_view_scroll() {
        let mut view = ThreadView::new();
        view.scroll_offset = 100.0;

        view.scroll_to_top();
        assert_eq!(view.scroll_offset, 0.0);
    }
}
