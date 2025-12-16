//! Modal dialog component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Modal dialog component.
pub struct Modal {
    title: String,
    content: String,
    animator: HudAnimator,

    // Styling
    color: Hsla,
    text_color: Hsla,
    backdrop_color: Hsla,
    padding: f32,
    corner_size: f32,
}

impl Modal {
    /// Create a new modal with title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: String::new(),
            animator: HudAnimator::new().enter_duration(20),
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            backdrop_color: Hsla::new(0.0, 0.0, 0.0, 0.7),
            padding: 24.0,
            corner_size: 16.0,
        }
    }

    /// Set modal content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set frame color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// Set backdrop color.
    pub fn backdrop_color(mut self, color: Hsla) -> Self {
        self.backdrop_color = color;
        self
    }

    /// Start enter animation (open modal).
    pub fn open(&mut self) {
        self.animator.enter();
    }

    /// Start exit animation (close modal).
    pub fn close(&mut self) {
        self.animator.exit();
    }

    /// Check if modal is open.
    pub fn is_open(&self) -> bool {
        self.animator.state().is_entered() || self.animator.state().is_animating()
    }

    /// Check if modal is visible.
    pub fn is_visible(&self) -> bool {
        self.animator.progress() > 0.0
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Check if a point is inside the modal content area.
    pub fn contains(&self, modal_bounds: Bounds, x: f32, y: f32) -> bool {
        modal_bounds.contains(Point::new(x, y))
    }

    /// Paint the modal.
    pub fn paint(
        &self,
        viewport: Bounds,
        modal_bounds: Bounds,
        scene: &mut Scene,
        text_system: &mut TextSystem,
    ) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        // Draw backdrop
        scene.draw_quad(wgpui::Quad::new(viewport).with_background(Hsla::new(
            self.backdrop_color.h,
            self.backdrop_color.s,
            self.backdrop_color.l,
            self.backdrop_color.a * progress,
        )));

        // Calculate animated modal bounds (scale in from center)
        let scale = 0.9 + 0.1 * progress;
        let scaled_width = modal_bounds.width() * scale;
        let scaled_height = modal_bounds.height() * scale;
        let scaled_x = modal_bounds.x() + (modal_bounds.width() - scaled_width) / 2.0;
        let scaled_y = modal_bounds.y() + (modal_bounds.height() - scaled_height) / 2.0;
        let scaled_bounds = Bounds::from_origin_size(
            Point::new(scaled_x, scaled_y),
            Size::new(scaled_width, scaled_height),
        );

        // Draw modal background
        scene.draw_quad(wgpui::Quad::new(scaled_bounds).with_background(Hsla::new(
            0.0,
            0.0,
            0.02,
            0.95 * progress,
        )));

        // Draw frame corners
        let corner = self.corner_size;
        let frame_color = Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            self.color.a * progress,
        );

        // Top-left corner
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(scaled_bounds.x(), scaled_bounds.y()),
                Size::new(corner, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(scaled_bounds.x(), scaled_bounds.y()),
                Size::new(1.0, corner),
            ))
            .with_background(frame_color),
        );

        // Top-right corner
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x() + scaled_bounds.width() - corner,
                    scaled_bounds.y(),
                ),
                Size::new(corner, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x() + scaled_bounds.width() - 1.0,
                    scaled_bounds.y(),
                ),
                Size::new(1.0, corner),
            ))
            .with_background(frame_color),
        );

        // Bottom-left corner
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x(),
                    scaled_bounds.y() + scaled_bounds.height() - 1.0,
                ),
                Size::new(corner, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x(),
                    scaled_bounds.y() + scaled_bounds.height() - corner,
                ),
                Size::new(1.0, corner),
            ))
            .with_background(frame_color),
        );

        // Bottom-right corner
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x() + scaled_bounds.width() - corner,
                    scaled_bounds.y() + scaled_bounds.height() - 1.0,
                ),
                Size::new(corner, 1.0),
            ))
            .with_background(frame_color),
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::from_origin_size(
                Point::new(
                    scaled_bounds.x() + scaled_bounds.width() - 1.0,
                    scaled_bounds.y() + scaled_bounds.height() - corner,
                ),
                Size::new(1.0, corner),
            ))
            .with_background(frame_color),
        );

        // Draw title
        let title_color = Hsla::new(
            self.text_color.h,
            self.text_color.s,
            self.text_color.l,
            self.text_color.a * progress,
        );
        let title_run = text_system.layout(
            &self.title,
            Point::new(
                scaled_bounds.x() + self.padding,
                scaled_bounds.y() + self.padding + 14.0,
            ),
            16.0,
            title_color,
        );
        scene.draw_text(title_run);

        // Draw title underline
        let underline_bounds = Bounds::from_origin_size(
            Point::new(
                scaled_bounds.x() + self.padding,
                scaled_bounds.y() + self.padding + 24.0,
            ),
            Size::new((scaled_bounds.width() - self.padding * 2.0) * progress, 1.0),
        );
        scene.draw_quad(
            wgpui::Quad::new(underline_bounds).with_background(Hsla::new(
                self.color.h,
                self.color.s,
                self.color.l,
                self.color.a * progress * 0.5,
            )),
        );

        // Draw content
        if !self.content.is_empty() {
            let content_color = Hsla::new(
                self.text_color.h,
                self.text_color.s,
                self.text_color.l,
                self.text_color.a * progress * 0.8,
            );
            let content_run = text_system.layout(
                &self.content,
                Point::new(
                    scaled_bounds.x() + self.padding,
                    scaled_bounds.y() + self.padding + 48.0,
                ),
                14.0,
                content_color,
            );
            scene.draw_text(content_run);
        }
    }
}
