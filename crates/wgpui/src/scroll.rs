//! Scroll container utilities for virtual scrolling.

use crate::geometry::{Bounds, Point, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollDirection {
    None,
    #[default]
    Vertical,
    Horizontal,
    Both,
}

pub struct ScrollContainer {
    pub bounds: Bounds,
    pub content_size: Size,
    pub scroll_offset: Point,
    pub direction: ScrollDirection,
}

impl ScrollContainer {
    pub fn new(bounds: Bounds, direction: ScrollDirection) -> Self {
        Self {
            bounds,
            content_size: Size::ZERO,
            scroll_offset: Point::ZERO,
            direction,
        }
    }

    pub fn vertical(bounds: Bounds) -> Self {
        Self::new(bounds, ScrollDirection::Vertical)
    }

    pub fn horizontal(bounds: Bounds) -> Self {
        Self::new(bounds, ScrollDirection::Horizontal)
    }

    pub fn set_viewport(&mut self, bounds: Bounds) {
        self.bounds = bounds;
        self.clamp_scroll();
    }

    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
        self.clamp_scroll();
    }

    pub fn scroll_by(&mut self, delta: Point) {
        match self.direction {
            ScrollDirection::None => {}
            ScrollDirection::Vertical => {
                self.scroll_offset.y += delta.y;
            }
            ScrollDirection::Horizontal => {
                self.scroll_offset.x += delta.x;
            }
            ScrollDirection::Both => {
                self.scroll_offset.x += delta.x;
                self.scroll_offset.y += delta.y;
            }
        }
        self.clamp_scroll();
    }

    pub fn scroll_to(&mut self, offset: Point) {
        self.scroll_offset = offset;
        self.clamp_scroll();
    }

    pub fn can_scroll(&self) -> bool {
        match self.direction {
            ScrollDirection::None => false,
            ScrollDirection::Vertical => self.content_size.height > self.bounds.height(),
            ScrollDirection::Horizontal => self.content_size.width > self.bounds.width(),
            ScrollDirection::Both => {
                self.content_size.height > self.bounds.height()
                    || self.content_size.width > self.bounds.width()
            }
        }
    }

    pub fn max_scroll(&self) -> Point {
        Point::new(
            (self.content_size.width - self.bounds.width()).max(0.0),
            (self.content_size.height - self.bounds.height()).max(0.0),
        )
    }

    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max.x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max.y);
    }
}

pub fn calculate_scrollbar_thumb(
    scroll: &ScrollContainer,
    is_vertical: bool,
    track_bounds: Bounds,
    min_thumb_size: f32,
) -> Option<Bounds> {
    let (viewport_size, content_size, scroll_offset, track_size) = if is_vertical {
        (
            scroll.bounds.height(),
            scroll.content_size.height,
            scroll.scroll_offset.y,
            track_bounds.height(),
        )
    } else {
        (
            scroll.bounds.width(),
            scroll.content_size.width,
            scroll.scroll_offset.x,
            track_bounds.width(),
        )
    };

    if content_size <= viewport_size {
        return None;
    }

    let thumb_ratio = viewport_size / content_size;
    let thumb_size = (track_size * thumb_ratio).max(min_thumb_size);
    let available_track = track_size - thumb_size;
    let max_scroll = content_size - viewport_size;
    let thumb_offset = (scroll_offset / max_scroll) * available_track;

    if is_vertical {
        Some(Bounds::new(
            track_bounds.origin.x,
            track_bounds.origin.y + thumb_offset,
            track_bounds.width(),
            thumb_size,
        ))
    } else {
        Some(Bounds::new(
            track_bounds.origin.x + thumb_offset,
            track_bounds.origin.y,
            thumb_size,
            track_bounds.height(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_container_vertical() {
        let mut scroll = ScrollContainer::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.set_content_size(Size::new(100.0, 500.0));

        assert!(scroll.can_scroll());
        assert_eq!(scroll.max_scroll(), Point::new(0.0, 300.0));
    }

    #[test]
    fn test_scroll_container_horizontal() {
        let mut scroll = ScrollContainer::horizontal(Bounds::new(0.0, 0.0, 200.0, 100.0));
        scroll.set_content_size(Size::new(500.0, 100.0));

        assert!(scroll.can_scroll());
        assert_eq!(scroll.max_scroll(), Point::new(300.0, 0.0));
    }

    #[test]
    fn test_scroll_by() {
        let mut scroll = ScrollContainer::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.set_content_size(Size::new(100.0, 500.0));

        scroll.scroll_by(Point::new(0.0, 50.0));
        assert_eq!(scroll.scroll_offset.y, 50.0);

        scroll.scroll_by(Point::new(0.0, 500.0));
        assert_eq!(scroll.scroll_offset.y, 300.0);
    }

    #[test]
    fn test_scroll_to() {
        let mut scroll = ScrollContainer::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.set_content_size(Size::new(100.0, 500.0));

        scroll.scroll_to(Point::new(0.0, 100.0));
        assert_eq!(scroll.scroll_offset.y, 100.0);

        scroll.scroll_to(Point::new(0.0, 1000.0));
        assert_eq!(scroll.scroll_offset.y, 300.0);
    }

    #[test]
    fn test_cannot_scroll_when_content_fits() {
        let mut scroll = ScrollContainer::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.set_content_size(Size::new(100.0, 100.0));

        assert!(!scroll.can_scroll());
    }

    #[test]
    fn test_scrollbar_thumb() {
        let mut scroll = ScrollContainer::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0));
        scroll.set_content_size(Size::new(100.0, 400.0));

        let track = Bounds::new(92.0, 0.0, 8.0, 200.0);
        let thumb = calculate_scrollbar_thumb(&scroll, true, track, 20.0);

        assert!(thumb.is_some());
        let thumb = thumb.unwrap();
        assert_eq!(thumb.height(), 100.0);
    }
}
