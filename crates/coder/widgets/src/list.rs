//! VirtualList widget - efficiently renders large lists.
//!
//! The VirtualList only renders items that are visible in the viewport,
//! enabling smooth scrolling of lists with thousands of items.

use crate::context::{EventContext, PaintContext};
use crate::widget::{EventResult, Widget, WidgetId};
use std::ops::Range;
use wgpui::scroll::ScrollContainer;
use wgpui::{Bounds, InputEvent, Point, Size};

/// Render function type for list items.
pub type RenderFn<T> = Box<dyn Fn(&T, usize, Bounds, &mut PaintContext)>;

/// A virtual list that efficiently renders large numbers of items.
pub struct VirtualList<T> {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Items in the list.
    items: Vec<T>,
    /// Fixed item height.
    item_height: f32,
    /// Render function.
    render_item: RenderFn<T>,
    /// Overscan (extra items to render above/below viewport).
    overscan: usize,
    /// Scroll container state.
    scroll: ScrollContainer,
}

impl<T: 'static> VirtualList<T> {
    /// Create a new virtual list with uniform item height.
    pub fn new<F>(items: Vec<T>, item_height: f32, render_item: F) -> Self
    where
        F: Fn(&T, usize, Bounds, &mut PaintContext) + 'static,
    {
        let content_height = item_height * items.len() as f32;
        let mut scroll = ScrollContainer::vertical(Bounds::ZERO);
        scroll.set_content_size(Size::new(0.0, content_height));

        Self {
            id: None,
            items,
            item_height,
            render_item: Box::new(render_item),
            overscan: 3,
            scroll,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the overscan count.
    pub fn overscan(mut self, count: usize) -> Self {
        self.overscan = count;
        self
    }

    /// Update the items.
    pub fn set_items(&mut self, items: Vec<T>) {
        let content_height = self.item_height * items.len() as f32;
        self.scroll.set_content_size(Size::new(0.0, content_height));
        self.items = items;
    }

    /// Get the items.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Calculate the total content height.
    pub fn content_height(&self) -> f32 {
        self.item_height * self.items.len() as f32
    }

    /// Calculate visible item range.
    fn visible_range(&self, viewport_height: f32, scroll_offset: f32) -> Range<usize> {
        let item_count = self.items.len();
        if item_count == 0 {
            return 0..0;
        }

        let first_visible = (scroll_offset / self.item_height).floor() as usize;
        let visible_count = (viewport_height / self.item_height).ceil() as usize + 1;

        let start = first_visible.saturating_sub(self.overscan);
        let end = (first_visible + visible_count + self.overscan).min(item_count);

        start..end
    }

    /// Scroll to show a specific item.
    pub fn scroll_to_item(&mut self, index: usize) {
        let y = index as f32 * self.item_height;
        self.scroll.scroll_to(Point::new(0.0, y));
    }
}

impl<T: 'static> Widget for VirtualList<T> {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Update scroll container
        self.scroll.set_viewport(bounds);

        // Calculate visible range
        let visible_range = self.visible_range(bounds.size.height, self.scroll.scroll_offset.y);

        // Push clip
        cx.scene.push_clip(bounds);

        // Render visible items
        for i in visible_range {
            if let Some(item) = self.items.get(i) {
                let y = i as f32 * self.item_height;

                let item_bounds = Bounds::new(
                    bounds.origin.x,
                    bounds.origin.y + y - self.scroll.scroll_offset.y,
                    bounds.size.width,
                    self.item_height,
                );

                // Only render if visible
                if item_bounds.origin.y + item_bounds.size.height >= bounds.origin.y
                    && item_bounds.origin.y <= bounds.origin.y + bounds.size.height
                {
                    (self.render_item)(item, i, item_bounds, cx);
                }
            }
        }

        cx.scene.pop_clip();
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::Wheel { delta, .. } => {
                if bounds.contains(Point::new(
                    bounds.origin.x + bounds.size.width / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0,
                )) {
                    self.scroll.scroll_by(*delta);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }
}

/// A simple list item that can be used with VirtualList.
pub trait VirtualListItem {
    /// Render this item.
    fn render(&self, bounds: Bounds, cx: &mut PaintContext);

    /// Get the height of this item.
    fn height(&self) -> f32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_list_creation() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let list = VirtualList::new(items, 40.0, |_item, _idx, _bounds, _cx| {
            // Render item
        });

        assert_eq!(list.item_height, 40.0);
        assert_eq!(list.content_height(), 120.0); // 3 items * 40px
    }

    #[test]
    fn test_visible_range() {
        let items: Vec<i32> = (0..100).collect();
        let list = VirtualList::new(items, 50.0, |_, _, _, _| {});

        // Viewport of 200px showing items at offset 0
        let range = list.visible_range(200.0, 0.0);
        assert_eq!(range.start, 0);
        assert!(range.end >= 4); // At least 4 items visible + overscan

        // Viewport of 200px scrolled to offset 250
        let range = list.visible_range(200.0, 250.0);
        assert!(range.start <= 5); // Should start around item 5
    }
}
