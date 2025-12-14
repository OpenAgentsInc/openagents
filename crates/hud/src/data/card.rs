//! Card - sci-fi styled content container with title and frame.

use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::frame::FrameCorners;
use crate::theme::hud;

/// Sci-fi styled card container.
///
/// Features:
/// - Animated frame border
/// - Optional title header
/// - Background with opacity
/// - Content area for child components
///
/// # Example
///
/// ```ignore
/// let mut card = Card::new()
///     .title("System Status")
///     .padding(15.0);
///
/// card.animator_mut().enter();
///
/// // In update:
/// card.tick();
///
/// // In paint:
/// card.paint(bounds, &mut scene, &mut text_system);
/// // Then paint child components within content_bounds()
/// ```
pub struct Card {
    title: Option<String>,
    animator: HudAnimator,
    frame: FrameCorners,

    // Styling
    corner_length: f32,
    padding: f32,
    title_height: f32,
    title_font_size: f32,
    bg_opacity: f32,
}

impl Card {
    /// Create a new card.
    pub fn new() -> Self {
        Self {
            title: None,
            animator: HudAnimator::new(),
            frame: FrameCorners::new()
                .corner_length(15.0)
                .line_width(1.0)
                .color(hud::FRAME_DIM),
            corner_length: 15.0,
            padding: 15.0,
            title_height: 35.0,
            title_font_size: 12.0,
            bg_opacity: 0.03,
        }
    }

    /// Set the card title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the corner length.
    pub fn corner_length(mut self, length: f32) -> Self {
        self.corner_length = length;
        self.frame = self.frame.corner_length(length);
        self
    }

    /// Set the padding.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set the background opacity.
    pub fn bg_opacity(mut self, opacity: f32) -> Self {
        self.bg_opacity = opacity;
        self
    }

    /// Set the frame color.
    pub fn frame_color(mut self, color: Hsla) -> Self {
        self.frame = self.frame.color(color);
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

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();
        self.frame.tick();

        // Sync frame animator
        if self.animator.state().is_visible() && !self.frame.animator().state().is_visible() {
            self.frame.animator_mut().enter();
        }
    }

    /// Calculate the content bounds (area inside padding and below title).
    pub fn content_bounds(&self, bounds: Bounds) -> Bounds {
        let y_offset = if self.title.is_some() {
            self.title_height
        } else {
            self.padding
        };

        Bounds::new(
            bounds.origin.x + self.padding,
            bounds.origin.y + y_offset,
            bounds.size.width - self.padding * 2.0,
            bounds.size.height - y_offset - self.padding,
        )
    }

    /// Paint the card.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Draw background
        let bg_color = Hsla::new(0.0, 0.0, 1.0, self.bg_opacity * progress);
        scene.draw_quad(wgpui::Quad::new(bounds).with_background(bg_color));

        // Draw frame
        self.frame.paint(bounds, scene);

        // Draw title if present
        if let Some(title) = &self.title {
            // Title background
            let title_bounds = Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                self.title_height,
            );
            scene.draw_quad(
                wgpui::Quad::new(title_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 1.0, 0.02 * progress))
            );

            // Title underline
            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    bounds.origin.x + self.corner_length,
                    bounds.origin.y + self.title_height - 1.0,
                    bounds.size.width - self.corner_length * 2.0,
                    1.0,
                ))
                .with_background(Hsla::new(
                    hud::FRAME_DIM.h,
                    hud::FRAME_DIM.s,
                    hud::FRAME_DIM.l,
                    hud::FRAME_DIM.a * 0.5 * progress,
                ))
            );

            // Title text
            let title_color = Hsla::new(
                hud::TEXT_MUTED.h,
                hud::TEXT_MUTED.s,
                hud::TEXT_MUTED.l,
                hud::TEXT_MUTED.a * progress,
            );
            let title_run = text_system.layout(
                &title.to_uppercase(),
                Point::new(
                    bounds.origin.x + self.padding,
                    bounds.origin.y + (self.title_height - self.title_font_size) / 2.0,
                ),
                self.title_font_size,
                title_color,
            );
            scene.draw_text(title_run);
        }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}
