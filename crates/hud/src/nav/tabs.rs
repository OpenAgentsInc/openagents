//! Tabbed navigation with animated indicator.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, TextSystem};

/// A single tab item.
#[derive(Clone, Debug)]
pub struct Tab {
    /// Tab label.
    pub label: String,
    /// Optional icon (rendered as text for now).
    pub icon: Option<String>,
    /// Whether tab is disabled.
    pub disabled: bool,
}

impl Tab {
    /// Create a new tab with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    /// Add an icon to the tab.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Mark tab as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Tabbed navigation component.
pub struct Tabs {
    tabs: Vec<Tab>,
    selected: usize,
    animator: HudAnimator,
    hovered: Option<usize>,

    // Animation state for indicator
    indicator_x: f32,
    indicator_width: f32,
    target_indicator_x: f32,
    target_indicator_width: f32,

    // Styling
    color: Hsla,
    text_color: Hsla,
    tab_padding: f32,
    tab_gap: f32,
    indicator_height: f32,
}

impl Tabs {
    /// Create tabs with items.
    pub fn new(tabs: Vec<Tab>) -> Self {
        Self {
            tabs,
            selected: 0,
            animator: HudAnimator::new().enter_duration(20),
            hovered: None,
            indicator_x: 0.0,
            indicator_width: 0.0,
            target_indicator_x: 0.0,
            target_indicator_width: 0.0,
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            tab_padding: 16.0,
            tab_gap: 4.0,
            indicator_height: 2.0,
        }
    }

    /// Set selected tab index.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index.min(self.tabs.len().saturating_sub(1));
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

        // Smoothly animate indicator position
        let lerp_speed = 0.15;
        self.indicator_x += (self.target_indicator_x - self.indicator_x) * lerp_speed;
        self.indicator_width += (self.target_indicator_width - self.indicator_width) * lerp_speed;
    }

    /// Handle mouse move, returns hovered tab index if any.
    pub fn on_mouse_move(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        if !bounds.contains(Point::new(x, y)) {
            self.hovered = None;
            return None;
        }

        let mut current_x = bounds.x();
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.disabled {
                current_x += self.calculate_tab_width(&tab.label) + self.tab_gap;
                continue;
            }

            let tab_width = self.calculate_tab_width(&tab.label);
            if x >= current_x && x < current_x + tab_width {
                self.hovered = Some(i);
                return Some(i);
            }
            current_x += tab_width + self.tab_gap;
        }

        self.hovered = None;
        None
    }

    /// Handle click, returns clicked tab index if any.
    pub fn on_click(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        if let Some(index) = self.on_mouse_move(bounds, x, y) {
            if !self.tabs[index].disabled {
                self.selected = index;
                return Some(index);
            }
        }
        None
    }

    /// Get currently selected tab index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    fn calculate_tab_width(&self, label: &str) -> f32 {
        // Approximate: 8px per character + padding
        (label.len() as f32 * 8.0) + (self.tab_padding * 2.0)
    }

    /// Paint the tabs.
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let base_alpha = self.color.a * progress;
        let text_alpha = self.text_color.a * progress;

        // Calculate tab positions and update indicator target
        let mut current_x = bounds.x();
        let mut tab_bounds: Vec<(Bounds, bool)> = Vec::new();

        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_width = self.calculate_tab_width(&tab.label);
            let tab_bound = Bounds::from_origin_size(
                Point::new(current_x, bounds.y()),
                wgpui::Size::new(tab_width, bounds.height() - self.indicator_height),
            );
            tab_bounds.push((tab_bound, tab.disabled));

            // Update indicator target for selected tab
            if i == self.selected {
                self.target_indicator_x = current_x;
                self.target_indicator_width = tab_width;

                // Initialize on first frame
                if self.indicator_width == 0.0 {
                    self.indicator_x = current_x;
                    self.indicator_width = tab_width;
                }
            }

            current_x += tab_width + self.tab_gap;
        }

        // Draw bottom line (full width)
        let line_bounds = Bounds::from_origin_size(
            Point::new(
                bounds.x(),
                bounds.y() + bounds.height() - self.indicator_height,
            ),
            wgpui::Size::new(bounds.width() * progress, self.indicator_height),
        );
        scene.draw_quad(wgpui::Quad::new(line_bounds).with_background(Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            base_alpha * 0.3,
        )));

        // Draw animated indicator
        let indicator_bounds = Bounds::from_origin_size(
            Point::new(
                self.indicator_x,
                bounds.y() + bounds.height() - self.indicator_height,
            ),
            wgpui::Size::new(self.indicator_width, self.indicator_height),
        );
        scene.draw_quad(
            wgpui::Quad::new(indicator_bounds).with_background(Hsla::new(
                self.color.h,
                self.color.s,
                self.color.l,
                base_alpha,
            )),
        );

        // Draw tab labels
        for (i, (tab_bound, disabled)) in tab_bounds.iter().enumerate() {
            let tab = &self.tabs[i];
            let is_selected = i == self.selected;
            let is_hovered = self.hovered == Some(i);

            let label_alpha = if *disabled {
                text_alpha * 0.3
            } else if is_selected {
                text_alpha
            } else if is_hovered {
                text_alpha * 0.8
            } else {
                text_alpha * 0.5
            };

            let label_color = Hsla::new(
                self.text_color.h,
                self.text_color.s,
                self.text_color.l,
                label_alpha,
            );

            // Center text in tab
            let text_x = tab_bound.x() + self.tab_padding;
            let text_y = tab_bound.y() + (tab_bound.height() / 2.0) + 4.0;

            let text_run =
                text_system.layout(&tab.label, Point::new(text_x, text_y), 14.0, label_color);
            scene.draw_text(text_run);
        }
    }
}
