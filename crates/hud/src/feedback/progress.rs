//! Progress bar component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size};

/// Progress bar style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ProgressStyle {
    /// Standard linear bar.
    #[default]
    Linear,
    /// Segmented bar.
    Segmented,
    /// Striped animated bar.
    Striped,
}

/// Animated progress bar.
pub struct Progress {
    value: f32,
    max_value: f32,
    style: ProgressStyle,
    animator: HudAnimator,

    // Animation state
    displayed_value: f32,
    stripe_offset: f32,

    // Styling
    color: Hsla,
    track_color: Hsla,
    height: f32,
    segment_count: usize,
    segment_gap: f32,
}

impl Progress {
    /// Create a new progress bar.
    pub fn new() -> Self {
        Self {
            value: 0.0,
            max_value: 100.0,
            style: ProgressStyle::Linear,
            animator: HudAnimator::new().enter_duration(20),
            displayed_value: 0.0,
            stripe_offset: 0.0,
            color: colors::FRAME_BRIGHT,
            track_color: Hsla::new(0.0, 0.0, 1.0, 0.1),
            height: 8.0,
            segment_count: 10,
            segment_gap: 2.0,
        }
    }

    /// Set current value.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.max(0.0).min(self.max_value);
        self
    }

    /// Set maximum value.
    pub fn max(mut self, max: f32) -> Self {
        self.max_value = max.max(0.0);
        self.value = self.value.min(self.max_value);
        self
    }

    /// Set progress style.
    pub fn style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    /// Set bar color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set bar height.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Set number of segments (for segmented style).
    pub fn segments(mut self, count: usize) -> Self {
        self.segment_count = count.max(2);
        self
    }

    /// Update the current value.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.max(0.0).min(self.max_value);
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

        // Smoothly animate value changes
        self.displayed_value += (self.value - self.displayed_value) * 0.1;

        // Animate stripes
        if self.style == ProgressStyle::Striped {
            self.stripe_offset += 0.5;
            if self.stripe_offset >= 20.0 {
                self.stripe_offset = 0.0;
            }
        }
    }

    /// Get current progress percentage (0.0 to 1.0).
    pub fn percentage(&self) -> f32 {
        if self.max_value > 0.0 {
            self.displayed_value / self.max_value
        } else {
            0.0
        }
    }

    /// Paint the progress bar.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = ease_out_cubic(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let bar_bounds = Bounds::from_origin_size(
            Point::new(bounds.x(), bounds.y() + (bounds.height() - self.height) / 2.0),
            Size::new(bounds.width(), self.height),
        );

        match self.style {
            ProgressStyle::Linear => self.paint_linear(bar_bounds, scene, progress),
            ProgressStyle::Segmented => self.paint_segmented(bar_bounds, scene, progress),
            ProgressStyle::Striped => self.paint_striped(bar_bounds, scene, progress),
        }
    }

    fn paint_linear(&self, bounds: Bounds, scene: &mut Scene, anim_progress: f32) {
        let track_alpha = self.track_color.a * anim_progress;
        let bar_alpha = self.color.a * anim_progress;

        // Draw track
        scene.draw_quad(
            wgpui::Quad::new(bounds)
                .with_background(Hsla::new(
                    self.track_color.h,
                    self.track_color.s,
                    self.track_color.l,
                    track_alpha,
                )),
        );

        // Draw filled portion
        let fill_width = bounds.width() * self.percentage() * anim_progress;
        if fill_width > 0.0 {
            let fill_bounds = Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y()),
                Size::new(fill_width, bounds.height()),
            );
            scene.draw_quad(
                wgpui::Quad::new(fill_bounds)
                    .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, bar_alpha)),
            );
        }
    }

    fn paint_segmented(&self, bounds: Bounds, scene: &mut Scene, anim_progress: f32) {
        let track_alpha = self.track_color.a * anim_progress;
        let bar_alpha = self.color.a * anim_progress;

        let total_gaps = (self.segment_count - 1) as f32 * self.segment_gap;
        let segment_width = (bounds.width() - total_gaps) / self.segment_count as f32;
        let filled_segments = (self.percentage() * self.segment_count as f32).ceil() as usize;

        for i in 0..self.segment_count {
            let x = bounds.x() + i as f32 * (segment_width + self.segment_gap);
            let segment_bounds = Bounds::from_origin_size(
                Point::new(x, bounds.y()),
                Size::new(segment_width, bounds.height()),
            );

            let is_filled = i < filled_segments;
            let color = if is_filled {
                Hsla::new(self.color.h, self.color.s, self.color.l, bar_alpha)
            } else {
                Hsla::new(self.track_color.h, self.track_color.s, self.track_color.l, track_alpha)
            };

            scene.draw_quad(wgpui::Quad::new(segment_bounds).with_background(color));
        }
    }

    fn paint_striped(&self, bounds: Bounds, scene: &mut Scene, anim_progress: f32) {
        let track_alpha = self.track_color.a * anim_progress;
        let bar_alpha = self.color.a * anim_progress;

        // Draw track
        scene.draw_quad(
            wgpui::Quad::new(bounds)
                .with_background(Hsla::new(
                    self.track_color.h,
                    self.track_color.s,
                    self.track_color.l,
                    track_alpha,
                )),
        );

        // Draw filled portion with stripes
        let fill_width = bounds.width() * self.percentage() * anim_progress;
        if fill_width > 0.0 {
            let fill_bounds = Bounds::from_origin_size(
                Point::new(bounds.x(), bounds.y()),
                Size::new(fill_width, bounds.height()),
            );

            // Base fill
            scene.draw_quad(
                wgpui::Quad::new(fill_bounds)
                    .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, bar_alpha)),
            );

            // Draw animated stripes
            let stripe_width = 10.0;
            let stripe_spacing = 20.0;
            let mut stripe_x = bounds.x() - stripe_spacing + self.stripe_offset;

            while stripe_x < bounds.x() + fill_width {
                if stripe_x + stripe_width > bounds.x() {
                    let start_x = stripe_x.max(bounds.x());
                    let end_x = (stripe_x + stripe_width).min(bounds.x() + fill_width);
                    let stripe_bounds = Bounds::from_origin_size(
                        Point::new(start_x, bounds.y()),
                        Size::new(end_x - start_x, bounds.height()),
                    );
                    scene.draw_quad(
                        wgpui::Quad::new(stripe_bounds)
                            .with_background(Hsla::new(
                                self.color.h,
                                self.color.s,
                                self.color.l.min(1.0) + 0.1,
                                bar_alpha * 0.3,
                            )),
                    );
                }
                stripe_x += stripe_spacing;
            }
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}
