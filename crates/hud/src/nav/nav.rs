//! Main navigation component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// A navigation item.
#[derive(Clone, Debug)]
pub struct NavItem {
    /// Item label.
    pub label: String,
    /// Optional icon (rendered as text).
    pub icon: Option<String>,
    /// Whether this item is active.
    pub active: bool,
    /// Whether this item is disabled.
    pub disabled: bool,
    /// Child items for nested navigation.
    pub children: Vec<NavItem>,
}

impl NavItem {
    /// Create a new nav item.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            active: false,
            disabled: false,
            children: Vec::new(),
        }
    }

    /// Add an icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Mark as active.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Mark as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Add child items.
    pub fn children(mut self, children: Vec<NavItem>) -> Self {
        self.children = children;
        self
    }
}

/// Navigation direction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NavDirection {
    /// Horizontal navigation (default).
    #[default]
    Horizontal,
    /// Vertical navigation (sidebar style).
    Vertical,
}

/// Main navigation component.
pub struct Nav {
    items: Vec<NavItem>,
    direction: NavDirection,
    animator: HudAnimator,
    hovered: Option<usize>,
    expanded: Option<usize>,

    // Item animation progress
    item_progress: Vec<f32>,

    // Styling
    color: Hsla,
    text_color: Hsla,
    item_padding: f32,
    item_gap: f32,
    indicator_width: f32,
}

impl Nav {
    /// Create navigation with items.
    pub fn new(items: Vec<NavItem>) -> Self {
        let item_count = items.len();
        Self {
            items,
            direction: NavDirection::Horizontal,
            animator: HudAnimator::new().enter_duration(20),
            hovered: None,
            expanded: None,
            item_progress: vec![0.0; item_count],
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            item_padding: 12.0,
            item_gap: 4.0,
            indicator_width: 3.0,
        }
    }

    /// Set navigation direction.
    pub fn direction(mut self, dir: NavDirection) -> Self {
        self.direction = dir;
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
            let stagger_threshold = i as f32 * 0.1;
            let target = if parent_progress > stagger_threshold {
                ((parent_progress - stagger_threshold) / (1.0 - stagger_threshold)).min(1.0)
            } else {
                0.0
            };
            *progress += (target - *progress) * 0.2;
        }
    }

    /// Handle mouse move, returns hovered item index if any.
    pub fn on_mouse_move(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        if !bounds.contains(Point::new(x, y)) {
            self.hovered = None;
            return None;
        }

        match self.direction {
            NavDirection::Horizontal => self.handle_horizontal_hover(bounds, x),
            NavDirection::Vertical => self.handle_vertical_hover(bounds, y),
        }
    }

    fn handle_horizontal_hover(&mut self, bounds: Bounds, x: f32) -> Option<usize> {
        let mut current_x = bounds.x();

        for (i, item) in self.items.iter().enumerate() {
            if item.disabled {
                current_x += self.calculate_item_width(&item.label) + self.item_gap;
                continue;
            }

            let item_width = self.calculate_item_width(&item.label);
            if x >= current_x && x < current_x + item_width {
                self.hovered = Some(i);
                return Some(i);
            }
            current_x += item_width + self.item_gap;
        }

        self.hovered = None;
        None
    }

    fn handle_vertical_hover(&mut self, bounds: Bounds, y: f32) -> Option<usize> {
        let item_height = self.calculate_item_height();
        let mut current_y = bounds.y();

        for (i, item) in self.items.iter().enumerate() {
            if item.disabled {
                current_y += item_height + self.item_gap;
                continue;
            }

            if y >= current_y && y < current_y + item_height {
                self.hovered = Some(i);
                return Some(i);
            }
            current_y += item_height + self.item_gap;
        }

        self.hovered = None;
        None
    }

    /// Handle click, returns clicked item index if any.
    pub fn on_click(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        if let Some(index) = self.on_mouse_move(bounds, x, y) {
            let item = &self.items[index];
            if !item.disabled {
                // Toggle expansion for items with children
                if !item.children.is_empty() {
                    self.expanded = if self.expanded == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                }
                return Some(index);
            }
        }
        None
    }

    fn calculate_item_width(&self, label: &str) -> f32 {
        (label.len() as f32 * 8.0) + (self.item_padding * 2.0)
    }

    fn calculate_item_height(&self) -> f32 {
        32.0
    }

    /// Paint the navigation.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let base_progress = self.animator.progress();
        if base_progress <= 0.0 {
            return;
        }

        match self.direction {
            NavDirection::Horizontal => self.paint_horizontal(bounds, scene, text_system),
            NavDirection::Vertical => self.paint_vertical(bounds, scene, text_system),
        }
    }

    fn paint_horizontal(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let mut current_x = bounds.x();
        let text_y = bounds.y() + (bounds.height() / 2.0) + 4.0;

        for (i, item) in self.items.iter().enumerate() {
            let item_progress = ease_out_cubic(self.item_progress.get(i).copied().unwrap_or(0.0));
            if item_progress <= 0.0 {
                continue;
            }

            let item_width = self.calculate_item_width(&item.label);
            let is_hovered = self.hovered == Some(i);

            // Determine colors
            let (text_alpha, indicator_alpha) = if item.disabled {
                (self.text_color.a * item_progress * 0.3, 0.0)
            } else if item.active {
                (self.text_color.a * item_progress, self.color.a * item_progress)
            } else if is_hovered {
                (self.text_color.a * item_progress * 0.8, self.color.a * item_progress * 0.5)
            } else {
                (self.text_color.a * item_progress * 0.5, 0.0)
            };

            // Draw icon if present
            let mut text_x = current_x + self.item_padding;
            if let Some(icon) = &item.icon {
                let icon_color = Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha);
                let icon_run = text_system.layout(icon, Point::new(text_x, text_y), 14.0, icon_color);
                scene.draw_text(icon_run);
                text_x += 20.0;
            }

            // Draw label
            let label_color = Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha);
            let label_run = text_system.layout(&item.label, Point::new(text_x, text_y), 14.0, label_color);
            scene.draw_text(label_run);

            // Draw bottom indicator for active/hovered
            if indicator_alpha > 0.0 {
                let indicator_bounds = Bounds::from_origin_size(
                    Point::new(current_x, bounds.y() + bounds.height() - 2.0),
                    Size::new(item_width * item_progress, 2.0),
                );
                scene.draw_quad(
                    wgpui::Quad::new(indicator_bounds)
                        .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, indicator_alpha)),
                );
            }

            current_x += item_width + self.item_gap;
        }
    }

    fn paint_vertical(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let item_height = self.calculate_item_height();
        let mut current_y = bounds.y();

        for (i, item) in self.items.iter().enumerate() {
            let item_progress = ease_out_cubic(self.item_progress.get(i).copied().unwrap_or(0.0));
            if item_progress <= 0.0 {
                continue;
            }

            let is_hovered = self.hovered == Some(i);

            // Determine colors
            let (text_alpha, indicator_alpha, bg_alpha) = if item.disabled {
                (self.text_color.a * item_progress * 0.3, 0.0, 0.0)
            } else if item.active {
                (self.text_color.a * item_progress, self.color.a * item_progress, self.color.a * item_progress * 0.1)
            } else if is_hovered {
                (self.text_color.a * item_progress * 0.8, self.color.a * item_progress * 0.5, self.color.a * item_progress * 0.05)
            } else {
                (self.text_color.a * item_progress * 0.5, 0.0, 0.0)
            };

            let item_bounds = Bounds::from_origin_size(
                Point::new(bounds.x(), current_y),
                Size::new(bounds.width(), item_height),
            );

            // Draw background
            if bg_alpha > 0.0 {
                scene.draw_quad(
                    wgpui::Quad::new(item_bounds)
                        .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, bg_alpha)),
                );
            }

            // Draw left indicator
            if indicator_alpha > 0.0 {
                let indicator_bounds = Bounds::from_origin_size(
                    Point::new(bounds.x(), current_y),
                    Size::new(self.indicator_width, item_height * item_progress),
                );
                scene.draw_quad(
                    wgpui::Quad::new(indicator_bounds)
                        .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, indicator_alpha)),
                );
            }

            // Draw icon if present
            let mut text_x = bounds.x() + self.item_padding + self.indicator_width;
            let text_y = current_y + (item_height / 2.0) + 4.0;

            if let Some(icon) = &item.icon {
                let icon_color = Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha);
                let icon_run = text_system.layout(icon, Point::new(text_x, text_y), 14.0, icon_color);
                scene.draw_text(icon_run);
                text_x += 24.0;
            }

            // Draw label
            let label_color = Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha);
            let label_run = text_system.layout(&item.label, Point::new(text_x, text_y), 14.0, label_color);
            scene.draw_text(label_run);

            // Draw expand indicator for items with children
            if !item.children.is_empty() {
                let expand_icon = if self.expanded == Some(i) { "v" } else { ">" };
                let expand_color = Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha * 0.5);
                let expand_run = text_system.layout(
                    expand_icon,
                    Point::new(bounds.x() + bounds.width() - self.item_padding - 8.0, text_y),
                    12.0,
                    expand_color,
                );
                scene.draw_text(expand_run);
            }

            current_y += item_height + self.item_gap;
        }
    }
}
