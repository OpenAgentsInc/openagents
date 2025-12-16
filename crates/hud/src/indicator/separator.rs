//! Separator/divider component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size};

/// Separator visual style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SeparatorStyle {
    /// Solid line.
    #[default]
    Solid,
    /// Dashed line.
    Dashed,
    /// Dotted line.
    Dotted,
    /// Line with center decoration.
    Decorated,
}

/// Horizontal or vertical separator/divider.
pub struct Separator {
    horizontal: bool,
    style: SeparatorStyle,
    animator: HudAnimator,

    // Styling
    color: Hsla,
    thickness: f32,
    dash_length: f32,
    gap_length: f32,
}

impl Separator {
    /// Create a new horizontal separator.
    pub fn horizontal() -> Self {
        Self {
            horizontal: true,
            style: SeparatorStyle::Solid,
            animator: HudAnimator::new().enter_duration(15),
            color: colors::FRAME_DIM,
            thickness: 1.0,
            dash_length: 8.0,
            gap_length: 4.0,
        }
    }

    /// Create a new vertical separator.
    pub fn vertical() -> Self {
        Self {
            horizontal: false,
            style: SeparatorStyle::Solid,
            animator: HudAnimator::new().enter_duration(15),
            color: colors::FRAME_DIM,
            thickness: 1.0,
            dash_length: 8.0,
            gap_length: 4.0,
        }
    }

    /// Set separator style.
    pub fn style(mut self, style: SeparatorStyle) -> Self {
        self.style = style;
        self
    }

    /// Set color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set thickness.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
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

    /// Paint the separator.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = ease_out_cubic(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let alpha = self.color.a * progress;
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, alpha);

        match self.style {
            SeparatorStyle::Solid => self.paint_solid(bounds, scene, color, progress),
            SeparatorStyle::Dashed => self.paint_dashed(bounds, scene, color, progress),
            SeparatorStyle::Dotted => self.paint_dotted(bounds, scene, color, progress),
            SeparatorStyle::Decorated => self.paint_decorated(bounds, scene, color, progress),
        }
    }

    fn paint_solid(&self, bounds: Bounds, scene: &mut Scene, color: Hsla, progress: f32) {
        if self.horizontal {
            let width = bounds.width() * progress;
            let x = bounds.x() + (bounds.width() - width) / 2.0;
            let y = bounds.y() + (bounds.height() - self.thickness) / 2.0;
            let line_bounds =
                Bounds::from_origin_size(Point::new(x, y), Size::new(width, self.thickness));
            scene.draw_quad(wgpui::Quad::new(line_bounds).with_background(color));
        } else {
            let height = bounds.height() * progress;
            let x = bounds.x() + (bounds.width() - self.thickness) / 2.0;
            let y = bounds.y() + (bounds.height() - height) / 2.0;
            let line_bounds =
                Bounds::from_origin_size(Point::new(x, y), Size::new(self.thickness, height));
            scene.draw_quad(wgpui::Quad::new(line_bounds).with_background(color));
        }
    }

    fn paint_dashed(&self, bounds: Bounds, scene: &mut Scene, color: Hsla, progress: f32) {
        if self.horizontal {
            let total_width = bounds.width() * progress;
            let start_x = bounds.x() + (bounds.width() - total_width) / 2.0;
            let y = bounds.y() + (bounds.height() - self.thickness) / 2.0;
            let mut x = start_x;

            while x < start_x + total_width {
                let dash_width = self.dash_length.min(start_x + total_width - x);
                let dash_bounds = Bounds::from_origin_size(
                    Point::new(x, y),
                    Size::new(dash_width, self.thickness),
                );
                scene.draw_quad(wgpui::Quad::new(dash_bounds).with_background(color));
                x += self.dash_length + self.gap_length;
            }
        } else {
            let total_height = bounds.height() * progress;
            let x = bounds.x() + (bounds.width() - self.thickness) / 2.0;
            let start_y = bounds.y() + (bounds.height() - total_height) / 2.0;
            let mut y = start_y;

            while y < start_y + total_height {
                let dash_height = self.dash_length.min(start_y + total_height - y);
                let dash_bounds = Bounds::from_origin_size(
                    Point::new(x, y),
                    Size::new(self.thickness, dash_height),
                );
                scene.draw_quad(wgpui::Quad::new(dash_bounds).with_background(color));
                y += self.dash_length + self.gap_length;
            }
        }
    }

    fn paint_dotted(&self, bounds: Bounds, scene: &mut Scene, color: Hsla, progress: f32) {
        let dot_size = self.thickness * 2.0;

        if self.horizontal {
            let total_width = bounds.width() * progress;
            let start_x = bounds.x() + (bounds.width() - total_width) / 2.0;
            let y = bounds.y() + (bounds.height() - dot_size) / 2.0;
            let mut x = start_x;

            while x < start_x + total_width {
                let dot_bounds =
                    Bounds::from_origin_size(Point::new(x, y), Size::new(dot_size, dot_size));
                scene.draw_quad(
                    wgpui::Quad::new(dot_bounds)
                        .with_background(color)
                        .with_uniform_radius(dot_size / 2.0),
                );
                x += dot_size + self.gap_length;
            }
        } else {
            let total_height = bounds.height() * progress;
            let x = bounds.x() + (bounds.width() - dot_size) / 2.0;
            let start_y = bounds.y() + (bounds.height() - total_height) / 2.0;
            let mut y = start_y;

            while y < start_y + total_height {
                let dot_bounds =
                    Bounds::from_origin_size(Point::new(x, y), Size::new(dot_size, dot_size));
                scene.draw_quad(
                    wgpui::Quad::new(dot_bounds)
                        .with_background(color)
                        .with_uniform_radius(dot_size / 2.0),
                );
                y += dot_size + self.gap_length;
            }
        }
    }

    fn paint_decorated(&self, bounds: Bounds, scene: &mut Scene, color: Hsla, progress: f32) {
        let decoration_size = 8.0;

        if self.horizontal {
            let total_width = bounds.width() * progress;
            let start_x = bounds.x() + (bounds.width() - total_width) / 2.0;
            let center_x = bounds.x() + bounds.width() / 2.0;
            let y = bounds.y() + (bounds.height() - self.thickness) / 2.0;

            // Left line
            let left_width = (center_x - decoration_size / 2.0 - start_x).max(0.0);
            if left_width > 0.0 {
                let left_bounds = Bounds::from_origin_size(
                    Point::new(start_x, y),
                    Size::new(left_width, self.thickness),
                );
                scene.draw_quad(wgpui::Quad::new(left_bounds).with_background(color));
            }

            // Right line
            let right_start = center_x + decoration_size / 2.0;
            let right_width = (start_x + total_width - right_start).max(0.0);
            if right_width > 0.0 {
                let right_bounds = Bounds::from_origin_size(
                    Point::new(right_start, y),
                    Size::new(right_width, self.thickness),
                );
                scene.draw_quad(wgpui::Quad::new(right_bounds).with_background(color));
            }

            // Center decoration (diamond)
            let dec_y = bounds.y() + (bounds.height() - decoration_size) / 2.0;
            let dec_bounds = Bounds::from_origin_size(
                Point::new(center_x - decoration_size / 2.0, dec_y),
                Size::new(decoration_size, decoration_size),
            );
            scene.draw_quad(wgpui::Quad::new(dec_bounds).with_border(color, 1.0));
        } else {
            // Vertical version
            let total_height = bounds.height() * progress;
            let x = bounds.x() + (bounds.width() - self.thickness) / 2.0;
            let start_y = bounds.y() + (bounds.height() - total_height) / 2.0;
            let center_y = bounds.y() + bounds.height() / 2.0;

            // Top line
            let top_height = (center_y - decoration_size / 2.0 - start_y).max(0.0);
            if top_height > 0.0 {
                let top_bounds = Bounds::from_origin_size(
                    Point::new(x, start_y),
                    Size::new(self.thickness, top_height),
                );
                scene.draw_quad(wgpui::Quad::new(top_bounds).with_background(color));
            }

            // Bottom line
            let bottom_start = center_y + decoration_size / 2.0;
            let bottom_height = (start_y + total_height - bottom_start).max(0.0);
            if bottom_height > 0.0 {
                let bottom_bounds = Bounds::from_origin_size(
                    Point::new(x, bottom_start),
                    Size::new(self.thickness, bottom_height),
                );
                scene.draw_quad(wgpui::Quad::new(bottom_bounds).with_background(color));
            }

            // Center decoration
            let dec_x = bounds.x() + (bounds.width() - decoration_size) / 2.0;
            let dec_bounds = Bounds::from_origin_size(
                Point::new(dec_x, center_y - decoration_size / 2.0),
                Size::new(decoration_size, decoration_size),
            );
            scene.draw_quad(wgpui::Quad::new(dec_bounds).with_border(color, 1.0));
        }
    }
}
