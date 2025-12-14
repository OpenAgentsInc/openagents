//! Tooltip component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Tooltip position relative to anchor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TooltipPosition {
    /// Above the anchor.
    #[default]
    Top,
    /// Below the anchor.
    Bottom,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
}

/// Hover tooltip component.
pub struct Tooltip {
    text: String,
    position: TooltipPosition,
    animator: HudAnimator,

    // Styling
    color: Hsla,
    text_color: Hsla,
    padding: f32,
    offset: f32,
}

impl Tooltip {
    /// Create a new tooltip with text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: TooltipPosition::Top,
            animator: HudAnimator::new().enter_duration(10).exit_duration(8),
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            padding: 8.0,
            offset: 8.0,
        }
    }

    /// Set tooltip position.
    pub fn position(mut self, pos: TooltipPosition) -> Self {
        self.position = pos;
        self
    }

    /// Set border color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// Update the text content.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Show the tooltip.
    pub fn show(&mut self) {
        self.animator.enter();
    }

    /// Hide the tooltip.
    pub fn hide(&mut self) {
        self.animator.exit();
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Check if tooltip is visible.
    pub fn is_visible(&self) -> bool {
        self.animator.progress() > 0.0
    }

    /// Calculate tooltip bounds based on anchor.
    pub fn calculate_bounds(&self, anchor: Bounds, text_system: &mut TextSystem) -> Bounds {
        let text_width = text_system.measure(&self.text, 12.0);
        let width = text_width + self.padding * 2.0;
        let height = 12.0 + self.padding * 2.0;

        let (x, y) = match self.position {
            TooltipPosition::Top => (
                anchor.x() + (anchor.width() - width) / 2.0,
                anchor.y() - height - self.offset,
            ),
            TooltipPosition::Bottom => (
                anchor.x() + (anchor.width() - width) / 2.0,
                anchor.y() + anchor.height() + self.offset,
            ),
            TooltipPosition::Left => (
                anchor.x() - width - self.offset,
                anchor.y() + (anchor.height() - height) / 2.0,
            ),
            TooltipPosition::Right => (
                anchor.x() + anchor.width() + self.offset,
                anchor.y() + (anchor.height() - height) / 2.0,
            ),
        };

        Bounds::from_origin_size(Point::new(x, y), Size::new(width, height))
    }

    /// Paint the tooltip at anchor position.
    pub fn paint(&self, anchor: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = ease_out_cubic(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let bounds = self.calculate_bounds(anchor, text_system);

        // Apply fade and slight offset animation
        let offset_y = (1.0 - progress) * 4.0;
        let animated_bounds = Bounds::from_origin_size(
            Point::new(bounds.x(), bounds.y() + offset_y),
            Size::new(bounds.width(), bounds.height()),
        );

        // Draw background
        scene.draw_quad(
            wgpui::Quad::new(animated_bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.05, 0.95 * progress))
                .with_border(
                    Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * progress),
                    1.0,
                ),
        );

        // Draw text
        let text_color = Hsla::new(
            self.text_color.h,
            self.text_color.s,
            self.text_color.l,
            self.text_color.a * progress,
        );
        let text_run = text_system.layout(
            &self.text,
            Point::new(animated_bounds.x() + self.padding, animated_bounds.y() + self.padding + 10.0),
            12.0,
            text_color,
        );
        scene.draw_text(text_run);
    }
}
