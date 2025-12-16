//! Figure/image container component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Image/figure container with frame and caption.
pub struct Figure {
    caption: Option<String>,
    animator: HudAnimator,

    // Styling
    color: Hsla,
    text_color: Hsla,
    corner_size: f32,
    padding: f32,
    show_scanlines: bool,
}

impl Figure {
    /// Create a new figure container.
    pub fn new() -> Self {
        Self {
            caption: None,
            animator: HudAnimator::new().enter_duration(20),
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT_MUTED,
            corner_size: 12.0,
            padding: 8.0,
            show_scanlines: true,
        }
    }

    /// Set caption text.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Set frame color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Show/hide scanline effect.
    pub fn scanlines(mut self, show: bool) -> Self {
        self.show_scanlines = show;
        self
    }

    /// Start enter animation.
    pub fn enter(&mut self) {
        self.animator.enter();
    }

    /// Start exit animation.
    pub fn exit(&mut self) {
        self.animator.exit();
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Get the inner content bounds (excluding frame and caption).
    pub fn content_bounds(&self, bounds: Bounds) -> Bounds {
        let caption_height = if self.caption.is_some() { 24.0 } else { 0.0 };
        Bounds::from_origin_size(
            Point::new(bounds.x() + self.padding, bounds.y() + self.padding),
            Size::new(
                bounds.width() - self.padding * 2.0,
                bounds.height() - self.padding * 2.0 - caption_height,
            ),
        )
    }

    /// Paint the figure frame (call before drawing content).
    pub fn paint_frame(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let frame_alpha = self.color.a * progress;
        let frame_color = Hsla::new(self.color.h, self.color.s, self.color.l, frame_alpha);

        // Draw background
        scene.draw_quad(wgpui::Quad::new(bounds).with_background(Hsla::new(
            0.0,
            0.0,
            0.0,
            0.5 * progress,
        )));

        // Draw corner frame
        let corner = self.corner_size;

        // Top-left
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y()),
                Size::new(corner * progress, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y()),
                Size::new(1.0, corner * progress),
            ))
            .with_background(frame_color),
        );

        // Top-right
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x() + bounds.width() - corner * progress, bounds.y()),
                Size::new(corner * progress, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x() + bounds.width() - 1.0, bounds.y()),
                Size::new(1.0, corner * progress),
            ))
            .with_background(frame_color),
        );

        // Bottom-left
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y() + bounds.height() - 1.0),
                Size::new(corner * progress, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y() + bounds.height() - corner * progress),
                Size::new(1.0, corner * progress),
            ))
            .with_background(frame_color),
        );

        // Bottom-right
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    bounds.x() + bounds.width() - corner * progress,
                    bounds.y() + bounds.height() - 1.0,
                ),
                Size::new(corner * progress, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    bounds.x() + bounds.width() - 1.0,
                    bounds.y() + bounds.height() - corner * progress,
                ),
                Size::new(1.0, corner * progress),
            ))
            .with_background(frame_color),
        );
    }

    /// Paint scanline overlay (call after drawing content).
    pub fn paint_overlay(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 || !self.show_scanlines {
            return;
        }

        let content = self.content_bounds(bounds);
        let scanline_spacing = 3.0;
        let mut y = content.y();

        while y < content.y() + content.height() {
            let scanline_bounds = Bounds::from_origin_size(
                Point::new(content.x(), y),
                Size::new(content.width(), 1.0),
            );
            scene.draw_quad(wgpui::Quad::new(scanline_bounds).with_background(Hsla::new(
                0.0,
                0.0,
                0.0,
                0.15 * progress,
            )));
            y += scanline_spacing;
        }
    }

    /// Paint caption (call last).
    pub fn paint_caption(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        if let Some(caption) = &self.caption {
            let caption_y = bounds.y() + bounds.height() - 20.0;
            let caption_color = Hsla::new(
                self.text_color.h,
                self.text_color.s,
                self.text_color.l,
                self.text_color.a * progress,
            );

            // Caption background
            let caption_bg = Bounds::from_origin_size(
                Point::new(bounds.x(), caption_y - 4.0),
                Size::new(bounds.width(), 24.0),
            );
            scene.draw_quad(wgpui::Quad::new(caption_bg).with_background(Hsla::new(
                0.0,
                0.0,
                0.0,
                0.7 * progress,
            )));

            // Caption text
            let caption_run = text_system.layout(
                caption,
                Point::new(bounds.x() + self.padding, caption_y + 8.0),
                11.0,
                caption_color,
            );
            scene.draw_text(caption_run);
        }
    }
}

impl Default for Figure {
    fn default() -> Self {
        Self::new()
    }
}
