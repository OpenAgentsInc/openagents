//! Scroll container utilities for virtual scrolling.

use crate::geometry::{Bounds, Point, Size};
use crate::scene::Scene;
use crate::{Hsla, Quad};

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

/// Helper for rendering scrollable content directly to a Scene.
///
/// Combines clipping + scroll offset adjustment for direct Scene drawing
/// without requiring the Component trait.
///
/// # Example
///
/// ```ignore
/// let mut region = ScrollRegion::vertical(viewport_bounds, content_height);
/// region.scroll_offset.y = state.scroll_offset;
///
/// region.begin(scene);  // push_clip
/// for (i, line) in lines.iter().enumerate() {
///     let y = region.scroll_y(base_y + i as f32 * line_height);
///     if region.is_visible_y(y, line_height) {
///         scene.draw_text(...);
///     }
/// }
/// region.end(scene);  // pop_clip
///
/// if region.can_scroll() {
///     region.draw_scrollbar(scene, track_bounds, thumb_color);
/// }
/// ```
pub struct ScrollRegion {
    /// Visible viewport bounds
    pub viewport: Bounds,
    /// Total content size (may exceed viewport)
    pub content_size: Size,
    /// Current scroll offset
    pub scroll_offset: Point,
    /// Scroll direction
    pub direction: ScrollDirection,
}

impl ScrollRegion {
    /// Create a new scroll region with the given viewport and direction.
    pub fn new(viewport: Bounds, content_size: Size, direction: ScrollDirection) -> Self {
        Self {
            viewport,
            content_size,
            scroll_offset: Point::ZERO,
            direction,
        }
    }

    /// Create a vertical scroll region.
    pub fn vertical(viewport: Bounds, content_height: f32) -> Self {
        Self::new(
            viewport,
            Size::new(viewport.size.width, content_height),
            ScrollDirection::Vertical,
        )
    }

    /// Create a horizontal scroll region.
    pub fn horizontal(viewport: Bounds, content_width: f32) -> Self {
        Self::new(
            viewport,
            Size::new(content_width, viewport.size.height),
            ScrollDirection::Horizontal,
        )
    }

    /// Begin rendering - pushes clip region to the scene.
    pub fn begin(&self, scene: &mut Scene) {
        scene.push_clip(self.viewport);
    }

    /// End rendering - pops clip region from the scene.
    pub fn end(&self, scene: &mut Scene) {
        scene.pop_clip();
    }

    /// Adjust a y-coordinate for scroll offset.
    /// Use this for positioning content within the scroll region.
    #[inline]
    pub fn scroll_y(&self, y: f32) -> f32 {
        y - self.scroll_offset.y
    }

    /// Adjust an x-coordinate for scroll offset.
    #[inline]
    pub fn scroll_x(&self, x: f32) -> f32 {
        x - self.scroll_offset.x
    }

    /// Adjust a point for scroll offset.
    #[inline]
    pub fn scroll_point(&self, p: Point) -> Point {
        Point::new(p.x - self.scroll_offset.x, p.y - self.scroll_offset.y)
    }

    /// Check if a y-coordinate range is visible in the viewport.
    pub fn is_visible_y(&self, y: f32, height: f32) -> bool {
        let viewport_top = self.viewport.origin.y;
        let viewport_bottom = viewport_top + self.viewport.size.height;
        y + height > viewport_top && y < viewport_bottom
    }

    /// Check if an x-coordinate range is visible in the viewport.
    pub fn is_visible_x(&self, x: f32, width: f32) -> bool {
        let viewport_left = self.viewport.origin.x;
        let viewport_right = viewport_left + self.viewport.size.width;
        x + width > viewport_left && x < viewport_right
    }

    /// Whether content exceeds viewport (scrolling is possible).
    pub fn can_scroll(&self) -> bool {
        match self.direction {
            ScrollDirection::None => false,
            ScrollDirection::Vertical => self.content_size.height > self.viewport.size.height,
            ScrollDirection::Horizontal => self.content_size.width > self.viewport.size.width,
            ScrollDirection::Both => {
                self.content_size.height > self.viewport.size.height
                    || self.content_size.width > self.viewport.size.width
            }
        }
    }

    /// Maximum scroll offset.
    pub fn max_scroll(&self) -> Point {
        Point::new(
            (self.content_size.width - self.viewport.size.width).max(0.0),
            (self.content_size.height - self.viewport.size.height).max(0.0),
        )
    }

    /// Scroll by delta, clamped to valid range.
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

    /// Scroll to a specific offset, clamped to valid range.
    pub fn scroll_to(&mut self, offset: Point) {
        self.scroll_offset = offset;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max.x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max.y);
    }

    /// Draw a vertical scrollbar to the scene.
    ///
    /// The track_bounds should be positioned where you want the scrollbar
    /// (typically on the right edge of the viewport).
    pub fn draw_scrollbar(
        &self,
        scene: &mut Scene,
        track_bounds: Bounds,
        track_color: Hsla,
        thumb_color: Hsla,
        corner_radius: f32,
    ) {
        // Draw track
        scene.draw_quad(
            Quad::new(track_bounds)
                .with_background(track_color)
                .with_corner_radius(corner_radius),
        );

        // Draw thumb
        let is_vertical = matches!(
            self.direction,
            ScrollDirection::Vertical | ScrollDirection::Both
        );

        // Create a temporary ScrollContainer to use the existing thumb calculation
        let scroll_container = ScrollContainer {
            bounds: self.viewport,
            content_size: self.content_size,
            scroll_offset: self.scroll_offset,
            direction: self.direction,
        };

        if let Some(thumb_bounds) =
            calculate_scrollbar_thumb(&scroll_container, is_vertical, track_bounds, 20.0)
        {
            scene.draw_quad(
                Quad::new(thumb_bounds)
                    .with_background(thumb_color)
                    .with_corner_radius(corner_radius),
            );
        }
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

    // ScrollRegion tests

    #[test]
    fn test_scroll_region_vertical() {
        let region = ScrollRegion::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0), 500.0);

        assert!(region.can_scroll());
        assert_eq!(region.max_scroll(), Point::new(0.0, 300.0));
    }

    #[test]
    fn test_scroll_region_horizontal() {
        let region = ScrollRegion::horizontal(Bounds::new(0.0, 0.0, 200.0, 100.0), 500.0);

        assert!(region.can_scroll());
        assert_eq!(region.max_scroll(), Point::new(300.0, 0.0));
    }

    #[test]
    fn test_scroll_region_scroll_y() {
        let mut region = ScrollRegion::vertical(Bounds::new(0.0, 100.0, 100.0, 200.0), 500.0);
        region.scroll_offset.y = 50.0;

        // scroll_y adjusts coordinate by subtracting offset
        assert_eq!(region.scroll_y(150.0), 100.0); // 150 - 50 = 100
        assert_eq!(region.scroll_y(200.0), 150.0); // 200 - 50 = 150
    }

    #[test]
    fn test_scroll_region_is_visible_y() {
        let region = ScrollRegion::vertical(Bounds::new(0.0, 100.0, 100.0, 200.0), 500.0);
        // Viewport is y=100 to y=300

        // Fully inside
        assert!(region.is_visible_y(150.0, 50.0)); // 150-200

        // Partially inside (top)
        assert!(region.is_visible_y(80.0, 50.0)); // 80-130

        // Partially inside (bottom)
        assert!(region.is_visible_y(280.0, 50.0)); // 280-330

        // Fully above
        assert!(!region.is_visible_y(0.0, 50.0)); // 0-50

        // Fully below
        assert!(!region.is_visible_y(400.0, 50.0)); // 400-450
    }

    #[test]
    fn test_scroll_region_scroll_by_clamped() {
        let mut region = ScrollRegion::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0), 500.0);

        region.scroll_by(Point::new(0.0, 50.0));
        assert_eq!(region.scroll_offset.y, 50.0);

        // Should clamp to max
        region.scroll_by(Point::new(0.0, 1000.0));
        assert_eq!(region.scroll_offset.y, 300.0);

        // Should clamp to min
        region.scroll_by(Point::new(0.0, -1000.0));
        assert_eq!(region.scroll_offset.y, 0.0);
    }

    #[test]
    fn test_scroll_region_cannot_scroll_when_content_fits() {
        let region = ScrollRegion::vertical(Bounds::new(0.0, 0.0, 100.0, 200.0), 100.0);
        assert!(!region.can_scroll());
    }

    #[test]
    fn test_scroll_region_scroll_point() {
        let mut region = ScrollRegion::new(
            Bounds::new(0.0, 0.0, 100.0, 200.0),
            Size::new(300.0, 500.0),
            ScrollDirection::Both,
        );
        region.scroll_offset = Point::new(10.0, 20.0);

        let adjusted = region.scroll_point(Point::new(50.0, 100.0));
        assert_eq!(adjusted, Point::new(40.0, 80.0));
    }
}
