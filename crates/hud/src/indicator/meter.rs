//! Meter/gauge component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Meter/gauge indicator.
pub struct Meter {
    value: f32,
    min_value: f32,
    max_value: f32,
    animator: HudAnimator,
    displayed_value: f32,

    // Labels
    label: Option<String>,
    show_value: bool,
    unit: Option<String>,

    // Thresholds for color changes
    warning_threshold: Option<f32>,
    critical_threshold: Option<f32>,

    // Styling
    color: Hsla,
    warning_color: Hsla,
    critical_color: Hsla,
    track_color: Hsla,
    height: f32,
    tick_count: usize,
}

impl Meter {
    /// Create a new meter.
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            animator: HudAnimator::new().enter_duration(20),
            displayed_value: 0.0,
            label: None,
            show_value: true,
            unit: None,
            warning_threshold: None,
            critical_threshold: None,
            color: colors::FRAME_BRIGHT,
            warning_color: Hsla::new(0.12, 0.8, 0.5, 0.9),
            critical_color: Hsla::new(0.0, 0.8, 0.5, 0.9),
            track_color: Hsla::new(0.0, 0.0, 1.0, 0.1),
            height: 16.0,
            tick_count: 10,
        }
    }

    /// Set current value.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.max(self.min_value).min(self.max_value);
        self
    }

    /// Set value range.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min_value = min;
        self.max_value = max;
        self.value = self.value.max(min).min(max);
        self
    }

    /// Set label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set unit suffix.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Show/hide value display.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Set warning threshold.
    pub fn warning_at(mut self, threshold: f32) -> Self {
        self.warning_threshold = Some(threshold);
        self
    }

    /// Set critical threshold.
    pub fn critical_at(mut self, threshold: f32) -> Self {
        self.critical_threshold = Some(threshold);
        self
    }

    /// Set main color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Update the current value.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.max(self.min_value).min(self.max_value);
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
    }

    /// Get the percentage (0.0 to 1.0).
    fn percentage(&self) -> f32 {
        let range = self.max_value - self.min_value;
        if range > 0.0 {
            (self.displayed_value - self.min_value) / range
        } else {
            0.0
        }
    }

    /// Get the appropriate color based on thresholds.
    fn current_color(&self) -> Hsla {
        if let Some(critical) = self.critical_threshold {
            if self.displayed_value >= critical {
                return self.critical_color;
            }
        }
        if let Some(warning) = self.warning_threshold {
            if self.displayed_value >= warning {
                return self.warning_color;
            }
        }
        self.color
    }

    /// Paint the meter.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let mut y = bounds.y();

        // Draw label if present
        if let Some(label) = &self.label {
            let label_color = Hsla::new(
                colors::TEXT.h,
                colors::TEXT.s,
                colors::TEXT.l,
                colors::TEXT.a * progress * 0.7,
            );
            let label_run = text_system.layout(label, Point::new(bounds.x(), y + 12.0), 11.0, label_color);
            scene.draw_text(label_run);
            y += 18.0;
        }

        // Draw track
        let track_bounds = Bounds::from_origin_size(
            Point::new(bounds.x(), y),
            Size::new(bounds.width(), self.height),
        );
        scene.draw_quad(
            wgpui::Quad::new(track_bounds)
                .with_background(Hsla::new(
                    self.track_color.h,
                    self.track_color.s,
                    self.track_color.l,
                    self.track_color.a * progress,
                )),
        );

        // Draw tick marks
        let tick_spacing = bounds.width() / self.tick_count as f32;
        for i in 0..=self.tick_count {
            let tick_x = bounds.x() + i as f32 * tick_spacing;
            let tick_height = if i % 2 == 0 { self.height } else { self.height * 0.5 };
            let tick_y = y + (self.height - tick_height) / 2.0;
            let tick_bounds = Bounds::from_origin_size(
                Point::new(tick_x, tick_y),
                Size::new(1.0, tick_height),
            );
            scene.draw_quad(
                wgpui::Quad::new(tick_bounds)
                    .with_background(Hsla::new(
                        colors::FRAME_DIM.h,
                        colors::FRAME_DIM.s,
                        colors::FRAME_DIM.l,
                        colors::FRAME_DIM.a * progress * 0.5,
                    )),
            );
        }

        // Draw filled portion
        let fill_width = bounds.width() * self.percentage() * progress;
        if fill_width > 0.0 {
            let fill_color = self.current_color();
            let fill_bounds = Bounds::from_origin_size(
                Point::new(bounds.x(), y),
                Size::new(fill_width, self.height),
            );
            scene.draw_quad(
                wgpui::Quad::new(fill_bounds)
                    .with_background(Hsla::new(fill_color.h, fill_color.s, fill_color.l, fill_color.a * progress)),
            );
        }

        // Draw value indicator line
        let indicator_x = bounds.x() + bounds.width() * self.percentage() * progress;
        let indicator_bounds = Bounds::from_origin_size(
            Point::new(indicator_x - 1.0, y - 2.0),
            Size::new(2.0, self.height + 4.0),
        );
        scene.draw_quad(
            wgpui::Quad::new(indicator_bounds)
                .with_background(Hsla::new(
                    colors::FRAME_BRIGHT.h,
                    colors::FRAME_BRIGHT.s,
                    colors::FRAME_BRIGHT.l,
                    colors::FRAME_BRIGHT.a * progress,
                )),
        );

        // Draw value if enabled
        if self.show_value {
            let value_text = if let Some(unit) = &self.unit {
                format!("{:.0}{}", self.displayed_value, unit)
            } else {
                format!("{:.0}", self.displayed_value)
            };
            let value_color = Hsla::new(
                colors::TEXT.h,
                colors::TEXT.s,
                colors::TEXT.l,
                colors::TEXT.a * progress,
            );
            let value_run = text_system.layout(
                &value_text,
                Point::new(bounds.x() + bounds.width() + 8.0, y + self.height / 2.0 + 4.0),
                12.0,
                value_color,
            );
            scene.draw_text(value_run);
        }
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}
