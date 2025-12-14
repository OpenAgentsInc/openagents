//! List - sci-fi styled list with animated items.

use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// A list item with text and optional icon marker.
#[derive(Clone)]
pub struct ListItem {
    /// Item text content.
    pub text: String,
    /// Optional secondary text.
    pub secondary: Option<String>,
}

impl ListItem {
    /// Create a new list item.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            secondary: None,
        }
    }

    /// Add secondary text.
    pub fn secondary(mut self, text: impl Into<String>) -> Self {
        self.secondary = Some(text.into());
        self
    }
}

/// Sci-fi styled list with staggered item animations.
///
/// Features:
/// - Staggered item reveal animation
/// - Bullet markers with glow effect
/// - Primary and secondary text support
///
/// # Example
///
/// ```ignore
/// let mut list = List::new()
///     .items(vec![
///         ListItem::new("Item 1"),
///         ListItem::new("Item 2").secondary("Details"),
///         ListItem::new("Item 3"),
///     ]);
///
/// list.animator_mut().enter();
///
/// // In update:
/// list.tick();
///
/// // In paint:
/// list.paint(bounds, &mut scene, &mut text_system);
/// ```
pub struct List {
    items: Vec<ListItem>,
    animator: HudAnimator,

    // Animation state per item
    item_progress: Vec<f32>,

    // Styling
    font_size: f32,
    secondary_font_size: f32,
    item_height: f32,
    marker_size: f32,
    marker_gap: f32,
    stagger_offset: f32,
}

impl List {
    /// Create a new empty list.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            animator: HudAnimator::new(),
            item_progress: Vec::new(),
            font_size: 14.0,
            secondary_font_size: 11.0,
            item_height: 28.0,
            marker_size: 4.0,
            marker_gap: 12.0,
            stagger_offset: 3.0, // frames between each item
        }
    }

    /// Set the list items.
    pub fn items(mut self, items: Vec<ListItem>) -> Self {
        self.item_progress = vec![0.0; items.len()];
        self.items = items;
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the item height.
    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = height;
        self
    }

    /// Set the stagger offset (frames between items).
    pub fn stagger_offset(mut self, frames: usize) -> Self {
        self.stagger_offset = frames as f32;
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

        let parent_progress = self.animator.progress();

        // Update item progress with stagger based on parent progress
        let speed = 0.15;
        for (i, progress) in self.item_progress.iter_mut().enumerate() {
            // Stagger: each item starts later based on parent progress
            let stagger_threshold = i as f32 * 0.1; // 10% offset per item
            let target = if parent_progress > stagger_threshold {
                ((parent_progress - stagger_threshold) / (1.0 - stagger_threshold)).min(1.0)
            } else {
                0.0
            };

            // Smooth transition
            if *progress < target {
                *progress = (*progress + speed).min(target);
            } else if *progress > target {
                *progress = (*progress - speed).max(target);
            }
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self) -> Size {
        let height = self.items.len() as f32 * self.item_height;
        Size::new(200.0, height)
    }

    /// Paint the list.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let parent_progress = self.animator.progress();
        if parent_progress <= 0.0 {
            return;
        }

        for (i, item) in self.items.iter().enumerate() {
            let item_progress = self.item_progress.get(i).copied().unwrap_or(0.0);
            if item_progress <= 0.0 {
                continue;
            }

            let eased = easing::ease_out_expo(item_progress);
            let item_y = bounds.origin.y + i as f32 * self.item_height;

            // Slide in from left
            let slide_offset = 20.0 * (1.0 - eased);
            let item_x = bounds.origin.x + slide_offset;

            // Draw marker (bullet)
            let marker_y = item_y + (self.item_height - self.marker_size) / 2.0;
            let marker_color = Hsla::new(
                hud::FRAME_BRIGHT.h,
                hud::FRAME_BRIGHT.s,
                hud::FRAME_BRIGHT.l,
                hud::FRAME_BRIGHT.a * eased,
            );

            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    item_x, marker_y,
                    self.marker_size, self.marker_size,
                ))
                .with_background(marker_color)
            );

            // Draw primary text
            let text_x = item_x + self.marker_size + self.marker_gap;
            let text_y = if item.secondary.is_some() {
                item_y + 2.0
            } else {
                item_y + (self.item_height - self.font_size) / 2.0
            };

            let text_color = Hsla::new(
                hud::TEXT.h,
                hud::TEXT.s,
                hud::TEXT.l,
                hud::TEXT.a * eased,
            );

            let text_run = text_system.layout(
                &item.text,
                Point::new(text_x, text_y),
                self.font_size,
                text_color,
            );
            scene.draw_text(text_run);

            // Draw secondary text if present
            if let Some(secondary) = &item.secondary {
                let secondary_y = text_y + self.font_size + 2.0;
                let secondary_color = Hsla::new(
                    hud::TEXT_MUTED.h,
                    hud::TEXT_MUTED.s,
                    hud::TEXT_MUTED.l,
                    hud::TEXT_MUTED.a * eased,
                );

                let secondary_run = text_system.layout(
                    secondary,
                    Point::new(text_x, secondary_y),
                    self.secondary_font_size,
                    secondary_color,
                );
                scene.draw_text(secondary_run);
            }
        }
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}
