//! ResizablePane - A pane component with optional resize handles on edges and corners.
//!
//! ResizablePane wraps content and allows the user to resize it by dragging edges or corners.
//! The resizable behavior can be toggled with the `resizable` flag (defaults to true).

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, Size};

/// Which edge or corner is being resized
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    /// Returns true if this is a corner resize
    pub fn is_corner(&self) -> bool {
        matches!(
            self,
            ResizeEdge::TopLeft
                | ResizeEdge::TopRight
                | ResizeEdge::BottomLeft
                | ResizeEdge::BottomRight
        )
    }

    /// Returns true if this affects the horizontal dimension
    pub fn affects_width(&self) -> bool {
        matches!(
            self,
            ResizeEdge::Left
                | ResizeEdge::Right
                | ResizeEdge::TopLeft
                | ResizeEdge::TopRight
                | ResizeEdge::BottomLeft
                | ResizeEdge::BottomRight
        )
    }

    /// Returns true if this affects the vertical dimension
    pub fn affects_height(&self) -> bool {
        matches!(
            self,
            ResizeEdge::Top
                | ResizeEdge::Bottom
                | ResizeEdge::TopLeft
                | ResizeEdge::TopRight
                | ResizeEdge::BottomLeft
                | ResizeEdge::BottomRight
        )
    }
}

/// Callback type for resize events
pub type OnResize = Box<dyn FnMut(Size)>;

/// A pane that can be resized by dragging its edges or corners.
pub struct ResizablePane {
    id: Option<ComponentId>,
    /// Whether the pane is resizable (defaults to true)
    resizable: bool,
    /// Width of the resize handle hit area in pixels
    handle_size: f32,
    /// Whether to show visual resize handles
    show_handles: bool,
    /// Color of the resize handles when visible
    handle_color: Hsla,
    /// Color of the resize handles when hovered
    handle_hover_color: Hsla,
    /// Minimum width
    min_width: f32,
    /// Minimum height
    min_height: f32,
    /// Maximum width (None = unlimited)
    max_width: Option<f32>,
    /// Maximum height (None = unlimited)
    max_height: Option<f32>,
    /// Current size (if set, overrides bounds during paint)
    size: Option<Size>,
    /// Current drag state
    drag_state: Option<DragState>,
    /// Which edge is currently hovered
    hovered_edge: ResizeEdge,
    /// Callback when resize occurs
    on_resize: Option<OnResize>,
    /// Background color (optional)
    background: Option<Hsla>,
    /// Border color (optional)
    border_color: Option<Hsla>,
    /// Border width
    border_width: f32,
}

struct DragState {
    edge: ResizeEdge,
    start_mouse: Point,
    start_bounds: Bounds,
}

impl ResizablePane {
    pub fn new() -> Self {
        Self {
            id: None,
            resizable: true,
            handle_size: 8.0,
            show_handles: false,
            handle_color: Hsla::new(0.0, 0.0, 0.5, 0.3),
            handle_hover_color: Hsla::new(180.0, 0.6, 0.5, 0.6),
            min_width: 50.0,
            min_height: 50.0,
            max_width: None,
            max_height: None,
            size: None,
            drag_state: None,
            hovered_edge: ResizeEdge::None,
            on_resize: None,
            background: None,
            border_color: None,
            border_width: 1.0,
        }
    }

    /// Set the component ID
    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set whether the pane is resizable (default: true)
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Set the resize handle hit area size
    pub fn handle_size(mut self, size: f32) -> Self {
        self.handle_size = size.max(2.0);
        self
    }

    /// Set whether to show visual resize handles
    pub fn show_handles(mut self, show: bool) -> Self {
        self.show_handles = show;
        self
    }

    /// Set the handle color
    pub fn handle_color(mut self, color: Hsla) -> Self {
        self.handle_color = color;
        self
    }

    /// Set the handle hover color
    pub fn handle_hover_color(mut self, color: Hsla) -> Self {
        self.handle_hover_color = color;
        self
    }

    /// Set minimum width
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width.max(10.0);
        self
    }

    /// Set minimum height
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height.max(10.0);
        self
    }

    /// Set minimum size (convenience method)
    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_width = width.max(10.0);
        self.min_height = height.max(10.0);
        self
    }

    /// Set maximum width
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set maximum height
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Set maximum size (convenience method)
    pub fn max_size(mut self, width: f32, height: f32) -> Self {
        self.max_width = Some(width);
        self.max_height = Some(height);
        self
    }

    /// Set the initial/current size
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some(Size::new(width, height));
        self
    }

    /// Set the resize callback
    pub fn on_resize<F>(mut self, f: F) -> Self
    where
        F: FnMut(Size) + 'static,
    {
        self.on_resize = Some(Box::new(f));
        self
    }

    /// Expose the resize hit test for external layout logic.
    pub fn edge_at(&self, bounds: Bounds, point: Point) -> ResizeEdge {
        self.hit_test(bounds, point)
    }

    /// Compute resized bounds from a drag without mutating internal state.
    pub fn resize_bounds(
        &self,
        edge: ResizeEdge,
        start_bounds: Bounds,
        start_mouse: Point,
        current_mouse: Point,
    ) -> Bounds {
        let drag = DragState {
            edge,
            start_mouse,
            start_bounds,
        };
        self.calculate_new_bounds(&drag, current_mouse)
    }

    /// Set background color
    pub fn background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    /// Set border color
    pub fn border_color(mut self, color: Hsla) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Set border width
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    /// Get the current size (if set)
    pub fn current_size(&self) -> Option<Size> {
        self.size
    }

    /// Set the size programmatically
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.size = Some(Size::new(
            width.clamp(self.min_width, self.max_width.unwrap_or(f32::MAX)),
            height.clamp(self.min_height, self.max_height.unwrap_or(f32::MAX)),
        ));
    }

    /// Check if currently being resized
    pub fn is_resizing(&self) -> bool {
        self.drag_state.is_some()
    }

    /// Get the currently hovered edge
    pub fn hovered_edge(&self) -> ResizeEdge {
        self.hovered_edge
    }

    /// Determine which resize edge a point is over
    fn hit_test(&self, bounds: Bounds, point: Point) -> ResizeEdge {
        if !self.resizable {
            return ResizeEdge::None;
        }

        let hs = self.handle_size;
        let x = point.x;
        let y = point.y;
        let bx = bounds.origin.x;
        let by = bounds.origin.y;
        let bw = bounds.size.width;
        let bh = bounds.size.height;

        let on_left = x >= bx && x < bx + hs;
        let on_right = x > bx + bw - hs && x <= bx + bw;
        let on_top = y >= by && y < by + hs;
        let on_bottom = y > by + bh - hs && y <= by + bh;
        let in_x = x >= bx && x <= bx + bw;
        let in_y = y >= by && y <= by + bh;

        // Check corners first (higher priority)
        if on_top && on_left {
            return ResizeEdge::TopLeft;
        }
        if on_top && on_right {
            return ResizeEdge::TopRight;
        }
        if on_bottom && on_left {
            return ResizeEdge::BottomLeft;
        }
        if on_bottom && on_right {
            return ResizeEdge::BottomRight;
        }

        // Check edges
        if on_top && in_x {
            return ResizeEdge::Top;
        }
        if on_bottom && in_x {
            return ResizeEdge::Bottom;
        }
        if on_left && in_y {
            return ResizeEdge::Left;
        }
        if on_right && in_y {
            return ResizeEdge::Right;
        }

        ResizeEdge::None
    }

    /// Calculate new bounds based on drag
    fn calculate_new_bounds(&self, drag: &DragState, current_mouse: Point) -> Bounds {
        let dx = current_mouse.x - drag.start_mouse.x;
        let dy = current_mouse.y - drag.start_mouse.y;

        let mut new_x = drag.start_bounds.origin.x;
        let mut new_y = drag.start_bounds.origin.y;
        let mut new_w = drag.start_bounds.size.width;
        let mut new_h = drag.start_bounds.size.height;

        match drag.edge {
            ResizeEdge::Top => {
                new_y += dy;
                new_h -= dy;
            }
            ResizeEdge::Bottom => {
                new_h += dy;
            }
            ResizeEdge::Left => {
                new_x += dx;
                new_w -= dx;
            }
            ResizeEdge::Right => {
                new_w += dx;
            }
            ResizeEdge::TopLeft => {
                new_x += dx;
                new_w -= dx;
                new_y += dy;
                new_h -= dy;
            }
            ResizeEdge::TopRight => {
                new_w += dx;
                new_y += dy;
                new_h -= dy;
            }
            ResizeEdge::BottomLeft => {
                new_x += dx;
                new_w -= dx;
                new_h += dy;
            }
            ResizeEdge::BottomRight => {
                new_w += dx;
                new_h += dy;
            }
            ResizeEdge::None => {}
        }

        // Apply constraints
        let min_w = self.min_width;
        let min_h = self.min_height;
        let max_w = self.max_width.unwrap_or(f32::MAX);
        let max_h = self.max_height.unwrap_or(f32::MAX);

        // Clamp width
        if new_w < min_w {
            if drag.edge.affects_width()
                && matches!(
                    drag.edge,
                    ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft
                )
            {
                new_x = drag.start_bounds.origin.x + drag.start_bounds.size.width - min_w;
            }
            new_w = min_w;
        } else if new_w > max_w {
            if drag.edge.affects_width()
                && matches!(
                    drag.edge,
                    ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft
                )
            {
                new_x = drag.start_bounds.origin.x + drag.start_bounds.size.width - max_w;
            }
            new_w = max_w;
        }

        // Clamp height
        if new_h < min_h {
            if drag.edge.affects_height()
                && matches!(
                    drag.edge,
                    ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight
                )
            {
                new_y = drag.start_bounds.origin.y + drag.start_bounds.size.height - min_h;
            }
            new_h = min_h;
        } else if new_h > max_h {
            if drag.edge.affects_height()
                && matches!(
                    drag.edge,
                    ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight
                )
            {
                new_y = drag.start_bounds.origin.y + drag.start_bounds.size.height - max_h;
            }
            new_h = max_h;
        }

        Bounds::new(new_x, new_y, new_w, new_h)
    }

    /// Draw a resize handle at the given position
    fn draw_handle(&self, cx: &mut PaintContext, bounds: Bounds, edge: ResizeEdge) {
        let color = if self.hovered_edge == edge
            || self.drag_state.as_ref().map(|d| d.edge) == Some(edge)
        {
            self.handle_hover_color
        } else {
            self.handle_color
        };

        let hs = self.handle_size;
        let bx = bounds.origin.x;
        let by = bounds.origin.y;
        let bw = bounds.size.width;
        let bh = bounds.size.height;

        let handle_bounds = match edge {
            ResizeEdge::Top => Bounds::new(bx + hs, by, bw - hs * 2.0, hs),
            ResizeEdge::Bottom => Bounds::new(bx + hs, by + bh - hs, bw - hs * 2.0, hs),
            ResizeEdge::Left => Bounds::new(bx, by + hs, hs, bh - hs * 2.0),
            ResizeEdge::Right => Bounds::new(bx + bw - hs, by + hs, hs, bh - hs * 2.0),
            ResizeEdge::TopLeft => Bounds::new(bx, by, hs, hs),
            ResizeEdge::TopRight => Bounds::new(bx + bw - hs, by, hs, hs),
            ResizeEdge::BottomLeft => Bounds::new(bx, by + bh - hs, hs, hs),
            ResizeEdge::BottomRight => Bounds::new(bx + bw - hs, by + bh - hs, hs, hs),
            ResizeEdge::None => return,
        };

        cx.scene
            .draw_quad(Quad::new(handle_bounds).with_background(color));
    }
}

impl Default for ResizablePane {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ResizablePane {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Use stored size if available, otherwise use provided bounds
        let actual_bounds = if let Some(size) = self.size {
            Bounds::new(bounds.origin.x, bounds.origin.y, size.width, size.height)
        } else {
            bounds
        };

        // Draw background if set
        if let Some(bg) = self.background {
            cx.scene
                .draw_quad(Quad::new(actual_bounds).with_background(bg));
        }

        // Draw border if set
        if let Some(border) = self.border_color {
            cx.scene
                .draw_quad(Quad::new(actual_bounds).with_border(border, self.border_width));
        }

        // Draw resize handles if enabled and resizable
        if self.resizable && self.show_handles {
            self.draw_handle(cx, actual_bounds, ResizeEdge::Top);
            self.draw_handle(cx, actual_bounds, ResizeEdge::Bottom);
            self.draw_handle(cx, actual_bounds, ResizeEdge::Left);
            self.draw_handle(cx, actual_bounds, ResizeEdge::Right);
            self.draw_handle(cx, actual_bounds, ResizeEdge::TopLeft);
            self.draw_handle(cx, actual_bounds, ResizeEdge::TopRight);
            self.draw_handle(cx, actual_bounds, ResizeEdge::BottomLeft);
            self.draw_handle(cx, actual_bounds, ResizeEdge::BottomRight);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if !self.resizable {
            return EventResult::Ignored;
        }

        // Use stored size if available
        let actual_bounds = if let Some(size) = self.size {
            Bounds::new(bounds.origin.x, bounds.origin.y, size.width, size.height)
        } else {
            bounds
        };

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);

                if let Some(ref drag) = self.drag_state {
                    // Currently dragging - calculate new size
                    let new_bounds = self.calculate_new_bounds(drag, point);
                    let new_size = Size::new(new_bounds.size.width, new_bounds.size.height);

                    self.size = Some(new_size);

                    if let Some(on_resize) = &mut self.on_resize {
                        on_resize(new_size);
                    }

                    return EventResult::Handled;
                } else {
                    // Not dragging - update hovered edge
                    let edge = self.hit_test(actual_bounds, point);
                    if edge != self.hovered_edge {
                        self.hovered_edge = edge;
                        return EventResult::Handled;
                    }
                }
            }

            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    let edge = self.hit_test(actual_bounds, point);

                    if edge != ResizeEdge::None {
                        self.drag_state = Some(DragState {
                            edge,
                            start_mouse: point,
                            start_bounds: actual_bounds,
                        });
                        return EventResult::Handled;
                    }
                }
            }

            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left && self.drag_state.is_some() {
                    self.drag_state = None;
                    return EventResult::Handled;
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if let Some(size) = self.size {
            (Some(size.width), Some(size.height))
        } else {
            (None, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resizable_pane_new() {
        let pane = ResizablePane::new();
        assert!(pane.resizable);
        assert_eq!(pane.handle_size, 8.0);
        assert!(!pane.show_handles);
    }

    #[test]
    fn test_resizable_pane_builder() {
        let pane = ResizablePane::new()
            .with_id(42)
            .resizable(false)
            .handle_size(12.0)
            .show_handles(true)
            .min_size(100.0, 100.0)
            .max_size(500.0, 500.0);

        assert_eq!(pane.id, Some(42));
        assert!(!pane.resizable);
        assert_eq!(pane.handle_size, 12.0);
        assert!(pane.show_handles);
        assert_eq!(pane.min_width, 100.0);
        assert_eq!(pane.min_height, 100.0);
        assert_eq!(pane.max_width, Some(500.0));
        assert_eq!(pane.max_height, Some(500.0));
    }

    #[test]
    fn test_resize_edge_properties() {
        assert!(ResizeEdge::TopLeft.is_corner());
        assert!(ResizeEdge::TopRight.is_corner());
        assert!(ResizeEdge::BottomLeft.is_corner());
        assert!(ResizeEdge::BottomRight.is_corner());
        assert!(!ResizeEdge::Top.is_corner());
        assert!(!ResizeEdge::Left.is_corner());

        assert!(ResizeEdge::Left.affects_width());
        assert!(ResizeEdge::Right.affects_width());
        assert!(ResizeEdge::TopLeft.affects_width());
        assert!(!ResizeEdge::Top.affects_width());
        assert!(!ResizeEdge::Bottom.affects_width());

        assert!(ResizeEdge::Top.affects_height());
        assert!(ResizeEdge::Bottom.affects_height());
        assert!(ResizeEdge::TopLeft.affects_height());
        assert!(!ResizeEdge::Left.affects_height());
        assert!(!ResizeEdge::Right.affects_height());
    }

    #[test]
    fn test_hit_test() {
        let pane = ResizablePane::new().handle_size(10.0);
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

        // Test corners
        assert_eq!(
            pane.hit_test(bounds, Point::new(5.0, 5.0)),
            ResizeEdge::TopLeft
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(95.0, 5.0)),
            ResizeEdge::TopRight
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(5.0, 95.0)),
            ResizeEdge::BottomLeft
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(95.0, 95.0)),
            ResizeEdge::BottomRight
        );

        // Test edges
        assert_eq!(
            pane.hit_test(bounds, Point::new(50.0, 5.0)),
            ResizeEdge::Top
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(50.0, 95.0)),
            ResizeEdge::Bottom
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(5.0, 50.0)),
            ResizeEdge::Left
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(95.0, 50.0)),
            ResizeEdge::Right
        );

        // Test center (no edge)
        assert_eq!(
            pane.hit_test(bounds, Point::new(50.0, 50.0)),
            ResizeEdge::None
        );
    }

    #[test]
    fn test_set_size_with_constraints() {
        let mut pane = ResizablePane::new()
            .min_size(50.0, 50.0)
            .max_size(200.0, 200.0);

        // Set size within bounds
        pane.set_size(100.0, 100.0);
        assert_eq!(pane.current_size(), Some(Size::new(100.0, 100.0)));

        // Set size below minimum
        pane.set_size(10.0, 10.0);
        assert_eq!(pane.current_size(), Some(Size::new(50.0, 50.0)));

        // Set size above maximum
        pane.set_size(500.0, 500.0);
        assert_eq!(pane.current_size(), Some(Size::new(200.0, 200.0)));
    }

    #[test]
    fn test_resizable_false_disables_hit_test() {
        let pane = ResizablePane::new().resizable(false).handle_size(10.0);
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

        // All positions should return None when not resizable
        assert_eq!(
            pane.hit_test(bounds, Point::new(5.0, 5.0)),
            ResizeEdge::None
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(95.0, 95.0)),
            ResizeEdge::None
        );
        assert_eq!(
            pane.hit_test(bounds, Point::new(50.0, 5.0)),
            ResizeEdge::None
        );
    }

    #[test]
    fn test_default() {
        let pane = ResizablePane::default();
        assert!(pane.resizable);
    }
}
