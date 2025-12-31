//! Tooltip Component
//!
//! Provides contextual information on hover or focus.

use crate::components::{Component, ComponentId, EventContext, EventResult, PaintContext};
use crate::{Bounds, InputEvent, Point, Quad, Size, theme};

/// Position of tooltip relative to target
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipPosition {
    /// Above the target
    #[default]
    Top,
    /// Below the target
    Bottom,
    /// To the left of target
    Left,
    /// To the right of target
    Right,
    /// Auto-position based on available space
    Auto,
}

/// Arrow direction for tooltip
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Tooltip component
#[derive(Debug, Clone)]
pub struct Tooltip {
    id: Option<ComponentId>,
    /// Tooltip text content
    content: String,
    /// Target element bounds
    target_bounds: Option<Bounds>,
    /// Preferred position
    position: TooltipPosition,
    /// Whether tooltip is visible
    visible: bool,
    /// Delay before showing (in frames/ticks)
    show_delay: u32,
    /// Current delay counter
    delay_counter: u32,
    /// Maximum width before wrapping
    max_width: f32,
    /// Padding inside tooltip
    padding: f32,
    /// Arrow size
    arrow_size: f32,
    /// Offset from target
    offset: f32,
}

impl Tooltip {
    /// Create a new tooltip
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            target_bounds: None,
            position: TooltipPosition::default(),
            visible: false,
            show_delay: 30, // ~500ms at 60fps
            delay_counter: 0,
            max_width: 250.0,
            padding: 8.0,
            arrow_size: 6.0,
            offset: 8.0,
        }
    }

    /// Set component ID
    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set tooltip content
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set tooltip position
    pub fn position(mut self, position: TooltipPosition) -> Self {
        self.position = position;
        self
    }

    /// Set target bounds (the element being tooltipped)
    pub fn target(mut self, bounds: Bounds) -> Self {
        self.target_bounds = Some(bounds);
        self
    }

    /// Set show delay in frames
    pub fn delay(mut self, frames: u32) -> Self {
        self.show_delay = frames;
        self
    }

    /// Set maximum width
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// Show the tooltip
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the tooltip
    pub fn hide(&mut self) {
        self.visible = false;
        self.delay_counter = 0;
    }

    /// Check if tooltip is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Update hover state (call each frame while hovering)
    pub fn update_hover(&mut self, hovering: bool) {
        if hovering {
            self.delay_counter += 1;
            if self.delay_counter >= self.show_delay {
                self.visible = true;
            }
        } else {
            self.hide();
        }
    }

    /// Calculate tooltip bounds based on target and position
    fn calculate_bounds(&self, viewport: Bounds, text_size: Size) -> (Bounds, ArrowDirection) {
        let Some(target) = self.target_bounds else {
            return (Bounds::new(0.0, 0.0, 0.0, 0.0), ArrowDirection::Down);
        };

        let tooltip_width = (text_size.width + self.padding * 2.0).min(self.max_width);
        let tooltip_height = text_size.height + self.padding * 2.0;

        let target_center_x = target.origin.x + target.size.width / 2.0;
        let target_center_y = target.origin.y + target.size.height / 2.0;

        // Determine actual position, handling Auto
        let actual_position = if self.position == TooltipPosition::Auto {
            // Prefer top, but use bottom if not enough space
            let space_above = target.origin.y;
            let space_below = viewport.size.height - (target.origin.y + target.size.height);

            if space_above > tooltip_height + self.offset {
                TooltipPosition::Top
            } else if space_below > tooltip_height + self.offset {
                TooltipPosition::Bottom
            } else {
                // Try left/right
                let space_left = target.origin.x;
                let space_right = viewport.size.width - (target.origin.x + target.size.width);

                if space_right > tooltip_width + self.offset {
                    TooltipPosition::Right
                } else if space_left > tooltip_width + self.offset {
                    TooltipPosition::Left
                } else {
                    TooltipPosition::Top // Fallback
                }
            }
        } else {
            self.position
        };

        let (x, y, arrow_dir) = match actual_position {
            TooltipPosition::Top => {
                let x = (target_center_x - tooltip_width / 2.0)
                    .max(viewport.origin.x)
                    .min(viewport.origin.x + viewport.size.width - tooltip_width);
                let y = target.origin.y - tooltip_height - self.offset - self.arrow_size;
                (x, y, ArrowDirection::Down)
            }
            TooltipPosition::Bottom => {
                let x = (target_center_x - tooltip_width / 2.0)
                    .max(viewport.origin.x)
                    .min(viewport.origin.x + viewport.size.width - tooltip_width);
                let y = target.origin.y + target.size.height + self.offset + self.arrow_size;
                (x, y, ArrowDirection::Up)
            }
            TooltipPosition::Left => {
                let x = target.origin.x - tooltip_width - self.offset - self.arrow_size;
                let y = (target_center_y - tooltip_height / 2.0)
                    .max(viewport.origin.y)
                    .min(viewport.origin.y + viewport.size.height - tooltip_height);
                (x, y, ArrowDirection::Right)
            }
            TooltipPosition::Right => {
                let x = target.origin.x + target.size.width + self.offset + self.arrow_size;
                let y = (target_center_y - tooltip_height / 2.0)
                    .max(viewport.origin.y)
                    .min(viewport.origin.y + viewport.size.height - tooltip_height);
                (x, y, ArrowDirection::Left)
            }
            TooltipPosition::Auto => unreachable!(),
        };

        (Bounds::new(x, y, tooltip_width, tooltip_height), arrow_dir)
    }
}

impl Component for Tooltip {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.visible || self.content.is_empty() {
            return;
        }

        // Measure text
        let font_size = theme::font_size::SM;
        let text_width = self.content.len() as f32 * font_size * 0.55; // Approximate
        let text_height = font_size * 1.4;
        let text_size = Size::new(
            text_width.min(self.max_width - self.padding * 2.0),
            text_height,
        );

        let (tooltip_bounds, _arrow_dir) = self.calculate_bounds(bounds, text_size);

        // Draw tooltip background
        cx.scene.draw_quad(
            Quad::new(tooltip_bounds)
                .with_background(theme::bg::ELEVATED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Draw arrow (simplified as small quad)
        // In a full implementation, this would be a triangle
        let target = self.target_bounds.unwrap_or(bounds);
        let arrow_bounds = match _arrow_dir {
            ArrowDirection::Down => Bounds::new(
                target.origin.x + target.size.width / 2.0 - self.arrow_size / 2.0,
                tooltip_bounds.origin.y + tooltip_bounds.size.height,
                self.arrow_size,
                self.arrow_size,
            ),
            ArrowDirection::Up => Bounds::new(
                target.origin.x + target.size.width / 2.0 - self.arrow_size / 2.0,
                tooltip_bounds.origin.y - self.arrow_size,
                self.arrow_size,
                self.arrow_size,
            ),
            ArrowDirection::Right => Bounds::new(
                tooltip_bounds.origin.x + tooltip_bounds.size.width,
                target.origin.y + target.size.height / 2.0 - self.arrow_size / 2.0,
                self.arrow_size,
                self.arrow_size,
            ),
            ArrowDirection::Left => Bounds::new(
                tooltip_bounds.origin.x - self.arrow_size,
                target.origin.y + target.size.height / 2.0 - self.arrow_size / 2.0,
                self.arrow_size,
                self.arrow_size,
            ),
        };
        cx.scene
            .draw_quad(Quad::new(arrow_bounds).with_background(theme::bg::ELEVATED));

        // Draw text
        let text_x = tooltip_bounds.origin.x + self.padding;
        let text_y = tooltip_bounds.origin.y + self.padding + font_size * 0.8;
        let text_run = cx.text.layout(
            &self.content,
            Point::new(text_x, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        // Tooltip doesn't handle events directly
        // Parent component should call update_hover()
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
    use crate::{Bounds, Size};

    #[test]
    fn test_tooltip_creation() {
        let tooltip = Tooltip::new("Hello, World!");
        assert_eq!(tooltip.content, "Hello, World!");
        assert!(!tooltip.is_visible());
    }

    #[test]
    fn test_tooltip_position() {
        let tooltip = Tooltip::new("Test").position(TooltipPosition::Bottom);
        assert_eq!(tooltip.position, TooltipPosition::Bottom);
    }

    #[test]
    fn test_tooltip_visibility() {
        let mut tooltip = Tooltip::new("Test");

        tooltip.show();
        assert!(tooltip.is_visible());

        tooltip.hide();
        assert!(!tooltip.is_visible());
    }

    #[test]
    fn test_tooltip_hover_delay() {
        let mut tooltip = Tooltip::new("Test").delay(5);

        // Simulate hovering for 4 frames
        for _ in 0..4 {
            tooltip.update_hover(true);
            assert!(!tooltip.is_visible());
        }

        // 5th frame should show
        tooltip.update_hover(true);
        assert!(tooltip.is_visible());

        // Stop hovering should hide
        tooltip.update_hover(false);
        assert!(!tooltip.is_visible());
    }

    #[test]
    fn test_tooltip_target() {
        let target = Bounds::new(100.0, 100.0, 50.0, 30.0);
        let tooltip = Tooltip::new("Test").target(target);
        assert_eq!(tooltip.target_bounds, Some(target));
    }

    #[test]
    fn test_tooltip_max_width() {
        let tooltip = Tooltip::new("Test").max_width(300.0);
        assert_eq!(tooltip.max_width, 300.0);
    }

    #[test]
    fn test_tooltip_calculate_bounds_top() {
        let target = Bounds::new(150.0, 120.0, 40.0, 20.0);
        let tooltip = Tooltip::new("Test")
            .position(TooltipPosition::Top)
            .target(target);
        let viewport = Bounds::new(0.0, 0.0, 400.0, 300.0);
        let text_size = Size::new(80.0, 14.0);

        let (bounds, arrow) = tooltip.calculate_bounds(viewport, text_size);
        assert_eq!(arrow, ArrowDirection::Down);

        let tooltip_height = text_size.height + tooltip.padding * 2.0;
        let expected_y = target.origin.y - tooltip_height - tooltip.offset - tooltip.arrow_size;
        assert!((bounds.origin.y - expected_y).abs() < 0.01);
    }

    #[test]
    fn test_tooltip_calculate_bounds_auto_prefers_top() {
        let target = Bounds::new(160.0, 150.0, 40.0, 20.0);
        let tooltip = Tooltip::new("Test")
            .position(TooltipPosition::Auto)
            .target(target);
        let viewport = Bounds::new(0.0, 0.0, 400.0, 300.0);
        let text_size = Size::new(60.0, 14.0);

        let (_bounds, arrow) = tooltip.calculate_bounds(viewport, text_size);
        assert_eq!(arrow, ArrowDirection::Down);
    }

    #[test]
    fn test_tooltip_calculate_bounds_auto_prefers_bottom() {
        let target = Bounds::new(120.0, 5.0, 40.0, 20.0);
        let tooltip = Tooltip::new("Test")
            .position(TooltipPosition::Auto)
            .target(target);
        let viewport = Bounds::new(0.0, 0.0, 400.0, 200.0);
        let text_size = Size::new(80.0, 24.0);

        let (_bounds, arrow) = tooltip.calculate_bounds(viewport, text_size);
        assert_eq!(arrow, ArrowDirection::Up);
    }

    #[test]
    fn test_tooltip_calculate_bounds_auto_left_fallback() {
        let target = Bounds::new(110.0, 20.0, 20.0, 10.0);
        let tooltip = Tooltip::new("Test")
            .position(TooltipPosition::Auto)
            .target(target);
        let viewport = Bounds::new(0.0, 0.0, 140.0, 60.0);
        let text_size = Size::new(60.0, 40.0);

        let (_bounds, arrow) = tooltip.calculate_bounds(viewport, text_size);
        assert_eq!(arrow, ArrowDirection::Right);
    }

    #[test]
    fn test_tooltip_calculate_bounds_clamps_x() {
        let target = Bounds::new(2.0, 100.0, 20.0, 20.0);
        let tooltip = Tooltip::new("Wide tooltip")
            .position(TooltipPosition::Top)
            .target(target);
        let viewport = Bounds::new(0.0, 0.0, 200.0, 200.0);
        let text_size = Size::new(180.0, 14.0);

        let (bounds, _arrow) = tooltip.calculate_bounds(viewport, text_size);
        assert!(bounds.origin.x >= viewport.origin.x);
    }
}
