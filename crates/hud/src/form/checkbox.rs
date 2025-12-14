//! Checkbox - sci-fi styled checkbox with animated checkmark.

use wgpui::{Bounds, Hsla, InputEvent, MouseButton, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// Callback for checked state change.
pub type OnChange = Box<dyn FnMut(bool)>;

/// Sci-fi styled checkbox with animated checkmark.
///
/// Features:
/// - Animated checkmark that draws in on check
/// - Animated border glow on hover
/// - Optional label
///
/// # Example
///
/// ```ignore
/// let mut checkbox = Checkbox::new()
///     .label("Enable feature")
///     .on_change(|checked| println!("Checked: {}", checked));
///
/// checkbox.animator_mut().enter();
///
/// // In update:
/// checkbox.tick();
///
/// // In paint:
/// checkbox.paint(bounds, &mut scene, &mut text_system);
///
/// // In event handling:
/// checkbox.event(&event, bounds);
/// ```
pub struct Checkbox {
    checked: bool,
    label: Option<String>,
    animator: HudAnimator,

    // State
    hovered: bool,

    // Animation
    check_progress: f32,

    // Styling
    box_size: f32,
    font_size: f32,
    gap: f32,
    border_width: f32,

    // Callbacks
    on_change: Option<OnChange>,
}

impl Checkbox {
    /// Create a new checkbox.
    pub fn new() -> Self {
        Self {
            checked: false,
            label: None,
            animator: HudAnimator::new(),
            hovered: false,
            check_progress: 0.0,
            box_size: 18.0,
            font_size: 14.0,
            gap: 10.0,
            border_width: 1.5,
            on_change: None,
        }
    }

    /// Set the initial checked state.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self.check_progress = if checked { 1.0 } else { 0.0 };
        self
    }

    /// Set the label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the box size.
    pub fn box_size(mut self, size: f32) -> Self {
        self.box_size = size;
        self
    }

    /// Set the font size for the label.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the gap between checkbox and label.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Set the on_change callback.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Get the current checked state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Set the checked state programmatically.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Check if hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();

        // Animate check progress
        let target = if self.checked { 1.0 } else { 0.0 };
        let speed = 0.12;
        if self.check_progress < target {
            self.check_progress = (self.check_progress + speed).min(target);
        } else if self.check_progress > target {
            self.check_progress = (self.check_progress - speed).max(target);
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self, text_system: &mut TextSystem) -> Size {
        let mut width = self.box_size;
        let mut height = self.box_size;

        if let Some(label) = &self.label {
            let text_size = text_system.measure_size(label, self.font_size, None);
            width += self.gap + text_size.width;
            height = height.max(text_size.height);
        }

        Size::new(width, height)
    }

    /// Paint the checkbox.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Calculate box bounds (vertically centered)
        let box_y = bounds.origin.y + (bounds.size.height - self.box_size) / 2.0;
        let box_x = bounds.origin.x;
        let box_bounds = Bounds::new(box_x, box_y, self.box_size, self.box_size);

        // Draw box background on hover or checked
        let bg_alpha = if self.hovered { 0.08 } else if self.checked { 0.04 } else { 0.0 };
        if bg_alpha > 0.0 {
            scene.draw_quad(
                wgpui::Quad::new(box_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 1.0, bg_alpha * progress))
            );
        }

        // Draw box border
        let border_color = if self.hovered {
            hud::FRAME_BRIGHT
        } else if self.checked {
            hud::FRAME_NORMAL
        } else {
            hud::FRAME_DIM
        };

        scene.draw_quad(
            wgpui::Quad::new(box_bounds)
                .with_border(Hsla::new(
                    border_color.h,
                    border_color.s,
                    border_color.l,
                    border_color.a * progress,
                ), self.border_width)
        );

        // Draw checkmark
        if self.check_progress > 0.0 {
            let eased = easing::ease_out_expo(self.check_progress);
            let check_color = Hsla::new(
                hud::FRAME_BRIGHT.h,
                hud::FRAME_BRIGHT.s,
                hud::FRAME_BRIGHT.l,
                hud::FRAME_BRIGHT.a * progress * eased,
            );

            // Checkmark dimensions
            let padding = self.box_size * 0.25;
            let inner_x = box_x + padding;
            let inner_y = box_y + padding;
            let inner_w = self.box_size - padding * 2.0;
            let inner_h = self.box_size - padding * 2.0;

            // Draw checkmark as two lines (short leg and long leg)
            // Short leg: bottom-left to bottom-center
            let line_width = 2.0;

            // We'll approximate checkmark with quads since we don't have line primitives
            // Short diagonal (bottom-left going down-right)
            let short_len = inner_w * 0.35 * eased;
            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    inner_x, inner_y + inner_h * 0.5,
                    short_len, line_width,
                ))
                .with_background(check_color)
            );

            // Long diagonal (center going up-right)
            if eased > 0.3 {
                let long_progress = ((eased - 0.3) / 0.7).min(1.0);
                let long_len = inner_w * 0.65 * long_progress;
                scene.draw_quad(
                    wgpui::Quad::new(Bounds::new(
                        inner_x + inner_w * 0.35, inner_y + inner_h * 0.5 - line_width,
                        long_len, line_width,
                    ))
                    .with_background(check_color)
                );
            }
        }

        // Draw label
        if let Some(label) = &self.label {
            let text_x = bounds.origin.x + self.box_size + self.gap;
            let text_size = text_system.measure_size(label, self.font_size, None);
            let text_y = bounds.origin.y + (bounds.size.height - text_size.height) / 2.0;

            let text_color = if self.hovered {
                hud::FRAME_BRIGHT
            } else {
                hud::TEXT
            };

            let text_run = text_system.layout(
                label,
                Point::new(text_x, text_y),
                self.font_size,
                Hsla::new(
                    text_color.h,
                    text_color.s,
                    text_color.l,
                    text_color.a * progress,
                ),
            );
            scene.draw_text(text_run);
        }
    }

    /// Handle an input event.
    ///
    /// Returns `true` if the event was handled.
    pub fn event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        match event {
            InputEvent::MouseMove { position, .. } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(*position);
                was_hovered != self.hovered
            }

            InputEvent::MouseDown { position, button, .. } => {
                if *button == MouseButton::Left && bounds.contains(*position) {
                    self.checked = !self.checked;
                    if let Some(on_change) = &mut self.on_change {
                        on_change(self.checked);
                    }
                    true
                } else {
                    false
                }
            }

            _ => false,
        }
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}
