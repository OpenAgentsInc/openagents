//! FrameHeader - header section frame with decorations.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Header frame with top line and corner decorations.
///
/// Creates a frame suitable for section headers with a horizontal
/// line and small corner accents.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameHeader::new()
///     .line_width(2.0)
///     .accent_size(8.0)
///     .color(theme::hud::FRAME_NORMAL);
///
/// frame.animator_mut().enter();
///
/// // In update:
/// frame.tick();
///
/// // In paint:
/// frame.paint(bounds, &mut scene);
/// ```
pub struct FrameHeader {
    animator: HudAnimator,
    /// Line width.
    line_width: f32,
    /// Size of corner accent marks.
    accent_size: f32,
    /// Line color.
    color: Hsla,
    /// Whether to show bottom line too.
    show_bottom: bool,
}

impl Default for FrameHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameHeader {
    /// Create a new header frame.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            line_width: 1.0,
            accent_size: 10.0,
            color: hud::FRAME_NORMAL,
            show_bottom: false,
        }
    }

    /// Set line width.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Set accent size.
    pub fn accent_size(mut self, size: f32) -> Self {
        self.accent_size = size.max(2.0);
        self
    }

    /// Set line color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Enable bottom line.
    pub fn show_bottom(mut self, show: bool) -> Self {
        self.show_bottom = show;
        self
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Tick the animation.
    pub fn tick(&mut self) -> bool {
        self.animator.tick()
    }

    /// Paint the header frame.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            self.color.a * progress,
        );
        let w = self.line_width;
        let a = self.accent_size;

        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let width = bounds.size.width;
        let height = bounds.size.height;

        // Top line - animate from center outward
        let top_center = x + width / 2.0;
        let top_half = (width / 2.0) * progress;
        scene.draw_quad(
            Quad::new(Bounds::new(top_center - top_half, y, top_half * 2.0, w))
                .with_background(color),
        );

        // Top-left accent (vertical)
        let accent_height = a * progress;
        scene.draw_quad(Quad::new(Bounds::new(x, y, w, accent_height)).with_background(color));

        // Top-right accent (vertical)
        scene.draw_quad(
            Quad::new(Bounds::new(x + width - w, y, w, accent_height)).with_background(color),
        );

        // Bottom line and accents if enabled
        if self.show_bottom {
            let bottom_y = y + height - w;

            // Bottom line
            scene.draw_quad(
                Quad::new(Bounds::new(
                    top_center - top_half,
                    bottom_y,
                    top_half * 2.0,
                    w,
                ))
                .with_background(color),
            );

            // Bottom-left accent (vertical, going up)
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x,
                    bottom_y - accent_height + w,
                    w,
                    accent_height,
                ))
                .with_background(color),
            );

            // Bottom-right accent (vertical, going up)
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x + width - w,
                    bottom_y - accent_height + w,
                    w,
                    accent_height,
                ))
                .with_background(color),
            );
        }
    }
}
