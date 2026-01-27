//! ScrollView component - a scrollable container.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{AnyComponent, Component, ComponentId, EventResult};
use crate::scroll::{ScrollContainer, ScrollDirection, calculate_scrollbar_thumb};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, Size, theme};

pub struct ScrollView {
    id: Option<ComponentId>,
    direction: ScrollDirection,
    content: Option<AnyComponent>,
    content_size: Size,
    show_scrollbar: bool,
    scrollbar_width: f32,
    scroll: ScrollContainer,
    dragging_scrollbar: bool,
}

impl ScrollView {
    pub fn new() -> Self {
        Self {
            id: None,
            direction: ScrollDirection::Vertical,
            content: None,
            content_size: Size::ZERO,
            show_scrollbar: true,
            scrollbar_width: 8.0,
            scroll: ScrollContainer::vertical(Bounds::ZERO),
            dragging_scrollbar: false,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            direction: ScrollDirection::Horizontal,
            scroll: ScrollContainer::horizontal(Bounds::ZERO),
            ..Self::new()
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        self.scroll.direction = direction;
        self
    }

    pub fn content<C: Component + 'static>(mut self, component: C) -> Self {
        self.content = Some(AnyComponent::new(component));
        self
    }

    pub fn set_content<C: Component + 'static>(&mut self, component: C) {
        self.content = Some(AnyComponent::new(component));
    }

    pub fn content_size(mut self, size: Size) -> Self {
        self.content_size = size;
        self
    }

    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
    }

    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbar_width = width;
        self
    }

    pub fn scroll_offset(&self) -> Point {
        self.scroll.scroll_offset
    }

    pub fn scroll_to(&mut self, offset: Point) {
        self.scroll.scroll_to(offset);
    }

    fn paint_scrollbar(&self, bounds: Bounds, cx: &mut PaintContext) {
        let track_bounds = match self.direction {
            ScrollDirection::Vertical | ScrollDirection::Both => Bounds::new(
                bounds.origin.x + bounds.size.width - self.scrollbar_width,
                bounds.origin.y,
                self.scrollbar_width,
                bounds.size.height,
            ),
            ScrollDirection::Horizontal => Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - self.scrollbar_width,
                bounds.size.width,
                self.scrollbar_width,
            ),
            ScrollDirection::None => return,
        };

        let track_color = theme::bg::SURFACE;
        cx.scene.draw_quad(
            Quad::new(track_bounds)
                .with_background(track_color)
                .with_corner_radius(self.scrollbar_width / 2.0),
        );

        let is_vertical = matches!(
            self.direction,
            ScrollDirection::Vertical | ScrollDirection::Both
        );

        if let Some(thumb_bounds) =
            calculate_scrollbar_thumb(&self.scroll, is_vertical, track_bounds, 20.0)
        {
            cx.scene.draw_quad(
                Quad::new(thumb_bounds)
                    .with_background(theme::text::MUTED)
                    .with_corner_radius(self.scrollbar_width / 2.0),
            );
        }
    }

    fn is_on_scrollbar(&self, bounds: Bounds, x: f32, y: f32) -> bool {
        let scrollbar_bounds = match self.direction {
            ScrollDirection::Vertical | ScrollDirection::Both => Bounds::new(
                bounds.origin.x + bounds.size.width - self.scrollbar_width,
                bounds.origin.y,
                self.scrollbar_width,
                bounds.size.height,
            ),
            ScrollDirection::Horizontal => Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - self.scrollbar_width,
                bounds.size.width,
                self.scrollbar_width,
            ),
            ScrollDirection::None => return false,
        };

        scrollbar_bounds.contains(Point::new(x, y))
    }

    fn handle_scrollbar_drag(&mut self, bounds: Bounds, x: f32, y: f32) {
        let progress = match self.direction {
            ScrollDirection::Vertical | ScrollDirection::Both => {
                ((y - bounds.origin.y) / bounds.size.height).clamp(0.0, 1.0)
            }
            ScrollDirection::Horizontal => {
                ((x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0)
            }
            ScrollDirection::None => return,
        };

        let max = self.scroll.max_scroll();
        let offset = match self.direction {
            ScrollDirection::Vertical | ScrollDirection::Both => {
                Point::new(self.scroll.scroll_offset.x, max.y * progress)
            }
            ScrollDirection::Horizontal => {
                Point::new(max.x * progress, self.scroll.scroll_offset.y)
            }
            ScrollDirection::None => return,
        };

        self.scroll.scroll_to(offset);
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ScrollView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.scroll.set_viewport(bounds);
        self.scroll.set_content_size(self.content_size);

        cx.scene.push_clip(bounds);

        if let Some(content) = &mut self.content {
            let content_bounds = Bounds::new(
                bounds.origin.x - self.scroll.scroll_offset.x,
                bounds.origin.y - self.scroll.scroll_offset.y,
                self.content_size.width,
                self.content_size.height,
            );

            let old_offset = cx.scroll_offset;
            cx.scroll_offset = self.scroll.scroll_offset;

            content.paint(content_bounds, cx);

            cx.scroll_offset = old_offset;
        }

        cx.scene.pop_clip();

        if self.show_scrollbar && self.scroll.can_scroll() {
            self.paint_scrollbar(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::Scroll { dx, dy } => {
                self.scroll.scroll_by(Point::new(*dx, *dy));
                return EventResult::Handled;
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left
                    && self.show_scrollbar
                    && self.is_on_scrollbar(bounds, *x, *y)
                {
                    self.dragging_scrollbar = true;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left && self.dragging_scrollbar {
                    self.dragging_scrollbar = false;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseMove { x, y } => {
                if self.dragging_scrollbar {
                    self.handle_scrollbar_drag(bounds, *x, *y);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        if let Some(content) = &mut self.content {
            return content.event(event, bounds, cx);
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
    fn test_scroll_view_new() {
        let scroll = ScrollView::new();
        assert_eq!(scroll.direction, ScrollDirection::Vertical);
        assert!(scroll.show_scrollbar);
    }

    #[test]
    fn test_scroll_view_horizontal() {
        let scroll = ScrollView::horizontal();
        assert_eq!(scroll.direction, ScrollDirection::Horizontal);
    }

    #[test]
    fn test_scroll_view_builder() {
        let scroll = ScrollView::new()
            .with_id(42)
            .direction(ScrollDirection::Both)
            .content_size(Size::new(500.0, 1000.0))
            .show_scrollbar(false)
            .scrollbar_width(10.0);

        assert_eq!(Component::id(&scroll), Some(42));
        assert_eq!(scroll.direction, ScrollDirection::Both);
        assert!(!scroll.show_scrollbar);
        assert_eq!(scroll.scrollbar_width, 10.0);
    }

    #[test]
    fn test_scroll_view_offset() {
        let mut scroll = ScrollView::new().content_size(Size::new(100.0, 500.0));

        scroll
            .scroll
            .set_viewport(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.scroll.set_content_size(Size::new(100.0, 500.0));

        assert_eq!(scroll.scroll_offset(), Point::ZERO);

        scroll.scroll_to(Point::new(0.0, 100.0));
        assert_eq!(scroll.scroll_offset().y, 100.0);
    }
}
