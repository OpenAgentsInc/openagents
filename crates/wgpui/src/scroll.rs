//! Scroll container functionality for wgpui.
//!
//! This module provides scroll containers that track content size,
//! viewport size, and scroll offset for implementing scrollable regions.

use crate::geometry::{Bounds, Point, Size};
use crate::input::InputEvent;

/// Scroll direction constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    /// Allow both horizontal and vertical scrolling.
    #[default]
    Both,
    /// Only vertical scrolling.
    Vertical,
    /// Only horizontal scrolling.
    Horizontal,
    /// No scrolling.
    None,
}

/// A scroll container that manages scrollable content.
#[derive(Debug, Clone)]
pub struct ScrollContainer {
    /// The bounds of the visible viewport.
    pub viewport: Bounds,
    /// The total size of the content.
    pub content_size: Size,
    /// Current scroll offset (how much content is scrolled past the viewport).
    pub scroll_offset: Point,
    /// Scroll direction constraints.
    pub direction: ScrollDirection,
    /// Scroll sensitivity multiplier for wheel events.
    pub scroll_sensitivity: f32,
}

impl Default for ScrollContainer {
    fn default() -> Self {
        Self {
            viewport: Bounds::ZERO,
            content_size: Size::ZERO,
            scroll_offset: Point::ZERO,
            direction: ScrollDirection::Both,
            scroll_sensitivity: 1.0,
        }
    }
}

impl ScrollContainer {
    /// Create a new scroll container with the given viewport.
    pub fn new(viewport: Bounds) -> Self {
        Self {
            viewport,
            ..Default::default()
        }
    }

    /// Create a vertical-only scroll container.
    pub fn vertical(viewport: Bounds) -> Self {
        Self {
            viewport,
            direction: ScrollDirection::Vertical,
            ..Default::default()
        }
    }

    /// Create a horizontal-only scroll container.
    pub fn horizontal(viewport: Bounds) -> Self {
        Self {
            viewport,
            direction: ScrollDirection::Horizontal,
            ..Default::default()
        }
    }

    /// Set the content size.
    pub fn with_content_size(mut self, size: Size) -> Self {
        self.content_size = size;
        self
    }

    /// Set the scroll offset.
    pub fn with_scroll_offset(mut self, offset: Point) -> Self {
        self.scroll_offset = offset;
        self.clamp_scroll();
        self
    }

    /// Set scroll sensitivity.
    pub fn with_sensitivity(mut self, sensitivity: f32) -> Self {
        self.scroll_sensitivity = sensitivity;
        self
    }

    /// Update the viewport bounds.
    pub fn set_viewport(&mut self, viewport: Bounds) {
        self.viewport = viewport;
        self.clamp_scroll();
    }

    /// Update the content size.
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
        self.clamp_scroll();
    }

    /// Get the maximum scroll offset.
    pub fn max_scroll(&self) -> Point {
        let max_x = (self.content_size.width - self.viewport.size.width).max(0.0);
        let max_y = (self.content_size.height - self.viewport.size.height).max(0.0);
        Point::new(max_x, max_y)
    }

    /// Scroll by a delta amount.
    pub fn scroll_by(&mut self, delta: Point) {
        let delta = match self.direction {
            ScrollDirection::Both => delta,
            ScrollDirection::Vertical => Point::new(0.0, delta.y),
            ScrollDirection::Horizontal => Point::new(delta.x, 0.0),
            ScrollDirection::None => Point::ZERO,
        };

        self.scroll_offset.x -= delta.x * self.scroll_sensitivity;
        self.scroll_offset.y -= delta.y * self.scroll_sensitivity;
        self.clamp_scroll();
    }

    /// Scroll to a specific offset.
    pub fn scroll_to(&mut self, offset: Point) {
        self.scroll_offset = offset;
        self.clamp_scroll();
    }

    /// Scroll to ensure a bounds is visible.
    pub fn scroll_to_visible(&mut self, target: Bounds) {
        // Calculate how much we need to scroll to make target visible
        let viewport_end = Point::new(
            self.scroll_offset.x + self.viewport.size.width,
            self.scroll_offset.y + self.viewport.size.height,
        );

        let target_end = Point::new(
            target.origin.x + target.size.width,
            target.origin.y + target.size.height,
        );

        // Horizontal scrolling
        if matches!(
            self.direction,
            ScrollDirection::Both | ScrollDirection::Horizontal
        ) {
            if target.origin.x < self.scroll_offset.x {
                self.scroll_offset.x = target.origin.x;
            } else if target_end.x > viewport_end.x {
                self.scroll_offset.x = target_end.x - self.viewport.size.width;
            }
        }

        // Vertical scrolling
        if matches!(
            self.direction,
            ScrollDirection::Both | ScrollDirection::Vertical
        ) {
            if target.origin.y < self.scroll_offset.y {
                self.scroll_offset.y = target.origin.y;
            } else if target_end.y > viewport_end.y {
                self.scroll_offset.y = target_end.y - self.viewport.size.height;
            }
        }

        self.clamp_scroll();
    }

    /// Check if a point (in content coordinates) is visible.
    pub fn is_point_visible(&self, point: Point) -> bool {
        let visible_bounds = Bounds::new(
            self.scroll_offset.x,
            self.scroll_offset.y,
            self.viewport.size.width,
            self.viewport.size.height,
        );
        visible_bounds.contains(point)
    }

    /// Check if a bounds (in content coordinates) intersects the visible area.
    pub fn is_bounds_visible(&self, bounds: &Bounds) -> bool {
        let visible_bounds = Bounds::new(
            self.scroll_offset.x,
            self.scroll_offset.y,
            self.viewport.size.width,
            self.viewport.size.height,
        );
        visible_bounds.intersection(bounds).is_some()
    }

    /// Transform a point from viewport coordinates to content coordinates.
    pub fn viewport_to_content(&self, point: Point) -> Point {
        Point::new(
            point.x + self.scroll_offset.x - self.viewport.origin.x,
            point.y + self.scroll_offset.y - self.viewport.origin.y,
        )
    }

    /// Transform a point from content coordinates to viewport coordinates.
    pub fn content_to_viewport(&self, point: Point) -> Point {
        Point::new(
            point.x - self.scroll_offset.x + self.viewport.origin.x,
            point.y - self.scroll_offset.y + self.viewport.origin.y,
        )
    }

    /// Handle an input event and return whether it was consumed.
    pub fn handle_event(&mut self, event: &InputEvent) -> bool {
        match event {
            InputEvent::Wheel { delta, .. } => {
                let old_offset = self.scroll_offset;
                self.scroll_by(*delta);
                self.scroll_offset != old_offset
            }
            _ => false,
        }
    }

    /// Get the visible content bounds (the portion of content currently visible).
    pub fn visible_content_bounds(&self) -> Bounds {
        Bounds::new(
            self.scroll_offset.x,
            self.scroll_offset.y,
            self.viewport
                .size
                .width
                .min(self.content_size.width - self.scroll_offset.x),
            self.viewport
                .size
                .height
                .min(self.content_size.height - self.scroll_offset.y),
        )
    }

    /// Check if there is scrollable content (content larger than viewport).
    pub fn can_scroll(&self) -> bool {
        match self.direction {
            ScrollDirection::Both => {
                self.content_size.width > self.viewport.size.width
                    || self.content_size.height > self.viewport.size.height
            }
            ScrollDirection::Vertical => self.content_size.height > self.viewport.size.height,
            ScrollDirection::Horizontal => self.content_size.width > self.viewport.size.width,
            ScrollDirection::None => false,
        }
    }

    /// Get the scroll progress as a percentage (0.0 to 1.0).
    pub fn scroll_progress(&self) -> Point {
        let max = self.max_scroll();
        let x = if max.x > 0.0 {
            self.scroll_offset.x / max.x
        } else {
            0.0
        };
        let y = if max.y > 0.0 {
            self.scroll_offset.y / max.y
        } else {
            0.0
        };
        Point::new(x, y)
    }

    /// Clamp scroll offset to valid range.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max.x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max.y);
    }
}

/// Scrollbar appearance settings.
#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    /// Width of the scrollbar track.
    pub track_width: f32,
    /// Minimum thumb size.
    pub min_thumb_size: f32,
    /// Corner radius for the thumb.
    pub thumb_radius: f32,
    /// Whether to auto-hide scrollbars.
    pub auto_hide: bool,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            track_width: 8.0,
            min_thumb_size: 20.0,
            thumb_radius: 4.0,
            auto_hide: true,
        }
    }
}

/// Calculate the bounds and size of a scrollbar thumb.
pub fn calculate_scrollbar_thumb(
    container: &ScrollContainer,
    vertical: bool,
    track_bounds: Bounds,
    min_thumb_size: f32,
) -> Option<Bounds> {
    let (content_dim, viewport_dim, scroll_offset) = if vertical {
        (
            container.content_size.height,
            container.viewport.size.height,
            container.scroll_offset.y,
        )
    } else {
        (
            container.content_size.width,
            container.viewport.size.width,
            container.scroll_offset.x,
        )
    };

    if content_dim <= viewport_dim {
        return None; // No scrollbar needed
    }

    // Calculate thumb size proportional to viewport/content ratio
    let track_length = if vertical {
        track_bounds.size.height
    } else {
        track_bounds.size.width
    };

    let thumb_size = (viewport_dim / content_dim * track_length).max(min_thumb_size);
    let available_track = track_length - thumb_size;
    let scroll_progress = scroll_offset / (content_dim - viewport_dim);
    let thumb_offset = scroll_progress * available_track;

    if vertical {
        Some(Bounds::new(
            track_bounds.origin.x,
            track_bounds.origin.y + thumb_offset,
            track_bounds.size.width,
            thumb_size,
        ))
    } else {
        Some(Bounds::new(
            track_bounds.origin.x + thumb_offset,
            track_bounds.origin.y,
            thumb_size,
            track_bounds.size.height,
        ))
    }
}
