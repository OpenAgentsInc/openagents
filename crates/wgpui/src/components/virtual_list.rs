//! VirtualList component - efficiently renders large lists.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::scroll::ScrollContainer;
use crate::{Bounds, InputEvent, Point, Size};
use std::ops::Range;

pub type RenderFn<T> = Box<dyn Fn(&T, usize, Bounds, &mut PaintContext)>;

pub struct VirtualList<T> {
    id: Option<ComponentId>,
    items: Vec<T>,
    item_height: f32,
    render_item: RenderFn<T>,
    overscan: usize,
    scroll: ScrollContainer,
}

impl<T: 'static> VirtualList<T> {
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

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn overscan(mut self, count: usize) -> Self {
        self.overscan = count;
        self
    }

    pub fn set_items(&mut self, items: Vec<T>) {
        let content_height = self.item_height * items.len() as f32;
        self.scroll.set_content_size(Size::new(0.0, content_height));
        self.items = items;
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn content_height(&self) -> f32 {
        self.item_height * self.items.len() as f32
    }

    pub fn scroll_to_item(&mut self, index: usize) {
        let y = index as f32 * self.item_height;
        self.scroll.scroll_to(Point::new(0.0, y));
    }

    pub fn scroll_offset(&self) -> Point {
        self.scroll.scroll_offset
    }

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
}

impl<T: 'static> Component for VirtualList<T> {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.scroll.set_viewport(bounds);

        let visible_range = self.visible_range(bounds.size.height, self.scroll.scroll_offset.y);

        cx.scene.push_clip(bounds);

        for i in visible_range {
            if let Some(item) = self.items.get(i) {
                let y = i as f32 * self.item_height;

                let item_bounds = Bounds::new(
                    bounds.origin.x,
                    bounds.origin.y + y - self.scroll.scroll_offset.y,
                    bounds.size.width,
                    self.item_height,
                );

                if item_bounds.origin.y + item_bounds.size.height >= bounds.origin.y
                    && item_bounds.origin.y <= bounds.origin.y + bounds.size.height
                {
                    (self.render_item)(item, i, item_bounds, cx);
                }
            }
        }

        cx.scene.pop_clip();
    }

    fn event(
        &mut self,
        event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        if let InputEvent::Scroll { dx, dy } = event {
            self.scroll.scroll_by(Point::new(*dx, *dy));
            return EventResult::Handled;
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_list_new() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let list = VirtualList::new(items, 40.0, |_item, _idx, _bounds, _cx| {});

        assert_eq!(list.item_height, 40.0);
        assert_eq!(list.item_count(), 3);
        assert_eq!(list.content_height(), 120.0);
    }

    #[test]
    fn test_virtual_list_builder() {
        let items: Vec<i32> = (0..100).collect();
        let list = VirtualList::new(items, 30.0, |_, _, _, _| {})
            .with_id(42)
            .overscan(5);

        assert_eq!(Component::id(&list), Some(42));
        assert_eq!(list.overscan, 5);
    }

    #[test]
    fn test_visible_range() {
        let items: Vec<i32> = (0..100).collect();
        let list = VirtualList::new(items, 50.0, |_, _, _, _| {});

        let range = list.visible_range(200.0, 0.0);
        assert_eq!(range.start, 0);
        assert!(range.end >= 4);

        let range = list.visible_range(200.0, 250.0);
        assert!(range.start <= 5);
    }

    #[test]
    fn test_set_items() {
        let items = vec![1, 2, 3];
        let mut list = VirtualList::new(items, 40.0, |_, _, _, _| {});

        assert_eq!(list.item_count(), 3);

        list.set_items(vec![1, 2, 3, 4, 5]);
        assert_eq!(list.item_count(), 5);
        assert_eq!(list.content_height(), 200.0);
    }

    #[test]
    fn test_scroll_to_item() {
        let items: Vec<i32> = (0..100).collect();
        let mut list = VirtualList::new(items, 40.0, |_, _, _, _| {});

        list.scroll
            .set_viewport(Bounds::new(0.0, 0.0, 100.0, 200.0));

        list.scroll_to_item(10);
        assert_eq!(list.scroll_offset().y, 400.0);
    }

    #[test]
    fn test_empty_list() {
        let items: Vec<i32> = vec![];
        let list = VirtualList::new(items, 40.0, |_, _, _, _| {});

        assert_eq!(list.item_count(), 0);
        assert_eq!(list.content_height(), 0.0);

        let range = list.visible_range(200.0, 0.0);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 0);
    }
}
