//! ScrollView widget - a scrollable container.
//!
//! The ScrollView widget wraps content and provides
//! scrolling when the content exceeds the viewport.

use crate::context::{EventContext, PaintContext};
use crate::widget::{AnyWidget, EventResult, Widget, WidgetId};
use wgpui::scroll::{ScrollContainer, ScrollDirection, calculate_scrollbar_thumb};
use wgpui::{Bounds, InputEvent, MouseButton, Point, Quad, Size};

/// A scrollable container widget.
pub struct ScrollView {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Scroll direction.
    direction: ScrollDirection,
    /// The content widget.
    content: Option<AnyWidget>,
    /// Content size (must be set).
    content_size: Size,
    /// Show scrollbar.
    show_scrollbar: bool,
    /// Scrollbar width.
    scrollbar_width: f32,
    /// Scroll container state.
    scroll: ScrollContainer,
    /// Whether scrollbar is being dragged.
    dragging_scrollbar: bool,
}

impl ScrollView {
    /// Create a new vertical scroll view.
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

    /// Create a horizontal scroll view.
    pub fn horizontal() -> Self {
        Self {
            direction: ScrollDirection::Horizontal,
            scroll: ScrollContainer::horizontal(Bounds::ZERO),
            ..Self::new()
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the scroll direction.
    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        self.scroll.direction = direction;
        self
    }

    /// Set the content widget.
    pub fn content<W: Widget + 'static>(mut self, widget: W) -> Self {
        self.content = Some(AnyWidget::new(widget));
        self
    }

    /// Set the content size.
    pub fn content_size(mut self, size: Size) -> Self {
        self.content_size = size;
        self
    }

    /// Show or hide the scrollbar.
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set the scrollbar width.
    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbar_width = width;
        self
    }

    /// Get the current scroll offset.
    pub fn scroll_offset(&self) -> Point {
        self.scroll.scroll_offset
    }

    /// Scroll to a position.
    pub fn scroll_to(&mut self, offset: Point) {
        self.scroll.scroll_to(offset);
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Update scroll container
        self.scroll.set_viewport(bounds);
        self.scroll.set_content_size(self.content_size);

        // Push clip for scrolling content
        cx.scene.push_clip(bounds);

        // Paint content with scroll offset
        if let Some(content) = &mut self.content {
            // Create content bounds adjusted for scroll
            let content_bounds = Bounds::new(
                bounds.origin.x - self.scroll.scroll_offset.x,
                bounds.origin.y - self.scroll.scroll_offset.y,
                self.content_size.width,
                self.content_size.height,
            );

            // Update paint context scroll offset
            let old_offset = cx.scroll_offset;
            cx.scroll_offset = self.scroll.scroll_offset;

            content.paint(content_bounds, cx);

            cx.scroll_offset = old_offset;
        }

        cx.scene.pop_clip();

        // Draw scrollbar if needed
        if self.show_scrollbar && self.scroll.can_scroll() {
            self.paint_scrollbar(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
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
            InputEvent::MouseDown {
                position, button, ..
            } => {
                if *button == MouseButton::Left {
                    // Check if clicking on scrollbar
                    if self.show_scrollbar && self.is_on_scrollbar(bounds, *position) {
                        self.dragging_scrollbar = true;
                        return EventResult::Handled;
                    }
                }
            }
            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left && self.dragging_scrollbar {
                    self.dragging_scrollbar = false;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseMove { position, .. } => {
                if self.dragging_scrollbar {
                    self.handle_scrollbar_drag(bounds, *position);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        // Propagate to content with adjusted event
        let adjusted_event = self.adjust_event_for_scroll(event);
        if let Some(content) = &mut self.content {
            return content.event(&adjusted_event, bounds, cx);
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }
}

impl ScrollView {
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

        // Draw track
        cx.scene.draw_quad(
            Quad::new(track_bounds)
                .with_background(wgpui::theme::bg::CARD)
                .with_uniform_radius(self.scrollbar_width / 2.0),
        );

        // Calculate thumb
        if let Some(thumb_bounds) = calculate_scrollbar_thumb(
            &self.scroll,
            matches!(
                self.direction,
                ScrollDirection::Vertical | ScrollDirection::Both
            ),
            track_bounds,
            20.0, // min thumb size
        ) {
            cx.scene.draw_quad(
                Quad::new(thumb_bounds)
                    .with_background(wgpui::theme::text::MUTED)
                    .with_uniform_radius(self.scrollbar_width / 2.0),
            );
        }
    }

    fn is_on_scrollbar(&self, bounds: Bounds, position: Point) -> bool {
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

        scrollbar_bounds.contains(position)
    }

    fn handle_scrollbar_drag(&mut self, bounds: Bounds, position: Point) {
        let progress = match self.direction {
            ScrollDirection::Vertical | ScrollDirection::Both => {
                ((position.y - bounds.origin.y) / bounds.size.height).clamp(0.0, 1.0)
            }
            ScrollDirection::Horizontal => {
                ((position.x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0)
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

    fn adjust_event_for_scroll(&self, event: &InputEvent) -> InputEvent {
        match event {
            InputEvent::MouseDown {
                position,
                button,
                modifiers,
            } => InputEvent::MouseDown {
                position: Point::new(
                    position.x + self.scroll.scroll_offset.x,
                    position.y + self.scroll.scroll_offset.y,
                ),
                button: *button,
                modifiers: *modifiers,
            },
            InputEvent::MouseUp {
                position,
                button,
                modifiers,
            } => InputEvent::MouseUp {
                position: Point::new(
                    position.x + self.scroll.scroll_offset.x,
                    position.y + self.scroll.scroll_offset.y,
                ),
                button: *button,
                modifiers: *modifiers,
            },
            InputEvent::MouseMove {
                position,
                modifiers,
            } => InputEvent::MouseMove {
                position: Point::new(
                    position.x + self.scroll.scroll_offset.x,
                    position.y + self.scroll.scroll_offset.y,
                ),
                modifiers: *modifiers,
            },
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_view_creation() {
        let scroll = ScrollView::new()
            .id(1)
            .direction(ScrollDirection::Vertical)
            .show_scrollbar(true);

        assert_eq!(scroll.id, Some(1));
        assert_eq!(scroll.direction, ScrollDirection::Vertical);
        assert!(scroll.show_scrollbar);
    }
}
