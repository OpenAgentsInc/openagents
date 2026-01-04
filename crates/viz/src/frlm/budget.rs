//! Budget meter - shows FRLM budget usage

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

/// Budget meter showing spend vs limit
pub struct BudgetMeter {
    spent_sats: u64,
    limit_sats: u64,
    reserved_sats: u64,

    // Animation state
    display_spent: f32,
    display_reserved: f32,

    // Colors
    bg_color: Hsla,
    spent_color: Hsla,
    reserved_color: Hsla,
    warning_color: Hsla,
    critical_color: Hsla,
}

impl BudgetMeter {
    pub fn new() -> Self {
        Self {
            spent_sats: 0,
            limit_sats: 1000,
            reserved_sats: 0,
            display_spent: 0.0,
            display_reserved: 0.0,
            bg_color: Hsla::new(0.0, 0.0, 0.12, 1.0),
            spent_color: Hsla::new(145.0 / 360.0, 0.7, 0.45, 1.0),  // Green
            reserved_color: Hsla::new(200.0 / 360.0, 0.6, 0.5, 0.6),  // Blue (semi-transparent)
            warning_color: Hsla::new(45.0 / 360.0, 0.9, 0.55, 1.0),   // Orange
            critical_color: Hsla::new(0.0, 0.85, 0.5, 1.0),           // Red
        }
    }

    /// Set the budget limit in sats
    pub fn with_limit(mut self, limit_sats: u64) -> Self {
        self.limit_sats = limit_sats;
        self
    }

    /// Set the current spent amount
    pub fn set_spent(&mut self, spent_sats: u64) {
        self.spent_sats = spent_sats;
    }

    /// Set the reserved (pending) amount
    pub fn set_reserved(&mut self, reserved_sats: u64) {
        self.reserved_sats = reserved_sats;
    }

    /// Set both spent and reserved
    pub fn set_budget(&mut self, spent_sats: u64, reserved_sats: u64, limit_sats: u64) {
        self.spent_sats = spent_sats;
        self.reserved_sats = reserved_sats;
        self.limit_sats = limit_sats;
    }

    fn spent_ratio(&self) -> f32 {
        if self.limit_sats == 0 {
            return 0.0;
        }
        (self.spent_sats as f32 / self.limit_sats as f32).clamp(0.0, 1.0)
    }

    fn reserved_ratio(&self) -> f32 {
        if self.limit_sats == 0 {
            return 0.0;
        }
        (self.reserved_sats as f32 / self.limit_sats as f32).clamp(0.0, 1.0 - self.spent_ratio())
    }

    fn current_color(&self) -> Hsla {
        let total_ratio = self.spent_ratio() + self.reserved_ratio();
        if total_ratio >= 0.9 {
            self.critical_color
        } else if total_ratio >= 0.7 {
            self.warning_color
        } else {
            self.spent_color
        }
    }
}

impl Default for BudgetMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for BudgetMeter {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Animate towards targets
        let target_spent = self.spent_ratio();
        let target_reserved = self.reserved_ratio();

        self.display_spent += (target_spent - self.display_spent) * 0.15;
        self.display_reserved += (target_reserved - self.display_reserved) * 0.15;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.bg_color)
                .with_corner_radius(2.0)
        );

        // Reserved (pending) portion - drawn first (underneath)
        let reserved_width = bounds.size.width * (self.display_spent + self.display_reserved);
        if reserved_width > 0.001 {
            let reserved_bounds = Bounds {
                origin: bounds.origin,
                size: Size {
                    width: reserved_width,
                    height: bounds.size.height,
                },
            };
            cx.scene.draw_quad(
                Quad::new(reserved_bounds)
                    .with_background(self.reserved_color)
                    .with_corner_radius(2.0)
            );
        }

        // Spent portion - drawn on top
        let spent_width = bounds.size.width * self.display_spent;
        if spent_width > 0.001 {
            let spent_bounds = Bounds {
                origin: bounds.origin,
                size: Size {
                    width: spent_width,
                    height: bounds.size.height,
                },
            };
            cx.scene.draw_quad(
                Quad::new(spent_bounds)
                    .with_background(self.current_color())
                    .with_corner_radius(2.0)
            );
        }

        // Text label: "123 / 1000 sats"
        let label = format!("{} / {} sats", self.spent_sats, self.limit_sats);
        let text_color = Hsla::new(0.0, 0.0, 0.9, 1.0);
        let font_size = (bounds.size.height * 0.6).min(12.0);

        let text_run = cx.text.layout(
            &label,
            Point {
                x: bounds.origin.x + 4.0,
                y: bounds.origin.y + (bounds.size.height - font_size) / 2.0,
            },
            font_size,
            text_color,
        );
        cx.scene.draw_text(text_run);
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(120.0), Some(20.0))
    }
}
