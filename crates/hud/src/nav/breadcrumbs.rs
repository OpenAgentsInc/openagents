//! Breadcrumb navigation component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, TextSystem};

/// A single breadcrumb item.
#[derive(Clone, Debug)]
pub struct Crumb {
    /// Crumb label.
    pub label: String,
    /// Whether this is the current/active crumb.
    pub active: bool,
}

impl Crumb {
    /// Create a new crumb.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            active: false,
        }
    }

    /// Mark as active (current page).
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// Breadcrumb navigation showing path hierarchy.
pub struct Breadcrumbs {
    crumbs: Vec<Crumb>,
    animator: HudAnimator,
    hovered: Option<usize>,

    // Item animation progress
    item_progress: Vec<f32>,

    // Styling
    color: Hsla,
    text_color: Hsla,
    separator: String,
    item_gap: f32,
}

impl Breadcrumbs {
    /// Create breadcrumbs with items.
    pub fn new(crumbs: Vec<Crumb>) -> Self {
        let item_count = crumbs.len();
        Self {
            crumbs,
            animator: HudAnimator::new().enter_duration(15),
            hovered: None,
            item_progress: vec![0.0; item_count],
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            separator: "/".to_string(),
            item_gap: 8.0,
        }
    }

    /// Set separator character.
    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
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

        let parent_progress = self.animator.progress();

        // Stagger item animations
        for (i, progress) in self.item_progress.iter_mut().enumerate() {
            let stagger_threshold = i as f32 * 0.15;
            let target = if parent_progress > stagger_threshold {
                ((parent_progress - stagger_threshold) / (1.0 - stagger_threshold)).min(1.0)
            } else {
                0.0
            };
            *progress += (target - *progress) * 0.2;
        }
    }

    /// Handle mouse move, returns hovered crumb index if any.
    pub fn on_mouse_move(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        if !bounds.contains(Point::new(x, y)) {
            self.hovered = None;
            return None;
        }

        // Calculate crumb positions
        let mut current_x = bounds.x();
        for (i, crumb) in self.crumbs.iter().enumerate() {
            let crumb_width = self.calculate_crumb_width(&crumb.label);

            if x >= current_x && x < current_x + crumb_width {
                // Don't hover on active (current) crumb
                if !crumb.active {
                    self.hovered = Some(i);
                    return Some(i);
                }
            }

            current_x += crumb_width;

            // Add separator width
            if i < self.crumbs.len() - 1 {
                current_x += self.calculate_crumb_width(&self.separator) + self.item_gap * 2.0;
            }
        }

        self.hovered = None;
        None
    }

    /// Handle click, returns clicked crumb index if any.
    pub fn on_click(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        self.on_mouse_move(bounds, x, y)
    }

    fn calculate_crumb_width(&self, text: &str) -> f32 {
        text.len() as f32 * 8.0
    }

    /// Paint the breadcrumbs.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let base_progress = self.animator.progress();
        if base_progress <= 0.0 {
            return;
        }

        let mut current_x = bounds.x();
        let text_y = bounds.y() + (bounds.height() / 2.0) + 4.0;

        for (i, crumb) in self.crumbs.iter().enumerate() {
            let item_progress = ease_out_cubic(self.item_progress.get(i).copied().unwrap_or(0.0));
            if item_progress <= 0.0 {
                continue;
            }

            let is_hovered = self.hovered == Some(i);

            // Determine text color
            let text_alpha = if crumb.active {
                self.text_color.a * item_progress
            } else if is_hovered {
                self.text_color.a * item_progress * 0.9
            } else {
                self.text_color.a * item_progress * 0.5
            };

            let label_color = Hsla::new(
                self.text_color.h,
                self.text_color.s,
                self.text_color.l,
                text_alpha,
            );

            // Draw crumb label
            let text_run = text_system.layout(
                &crumb.label,
                Point::new(current_x, text_y),
                14.0,
                label_color,
            );
            scene.draw_text(text_run);

            // Draw underline on hover
            if is_hovered {
                let crumb_width = self.calculate_crumb_width(&crumb.label);
                let underline_bounds = Bounds::from_origin_size(
                    Point::new(current_x, text_y + 4.0),
                    wgpui::Size::new(crumb_width * item_progress, 1.0),
                );
                scene.draw_quad(
                    wgpui::Quad::new(underline_bounds).with_background(Hsla::new(
                        self.color.h,
                        self.color.s,
                        self.color.l,
                        self.color.a * item_progress * 0.5,
                    )),
                );
            }

            current_x += self.calculate_crumb_width(&crumb.label);

            // Draw separator (except after last item)
            if i < self.crumbs.len() - 1 {
                current_x += self.item_gap;

                let sep_color = Hsla::new(
                    self.text_color.h,
                    self.text_color.s,
                    self.text_color.l,
                    self.text_color.a * item_progress * 0.3,
                );

                let sep_run = text_system.layout(
                    &self.separator,
                    Point::new(current_x, text_y),
                    14.0,
                    sep_color,
                );
                scene.draw_text(sep_run);

                current_x += self.calculate_crumb_width(&self.separator) + self.item_gap;
            }
        }
    }
}
