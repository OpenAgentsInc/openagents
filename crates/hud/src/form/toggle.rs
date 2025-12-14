//! Toggle - sci-fi styled on/off switch.

use wgpui::{Bounds, Hsla, InputEvent, MouseButton, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// Callback for toggle state change.
pub type OnChange = Box<dyn FnMut(bool)>;

/// Sci-fi styled toggle switch.
///
/// Features:
/// - Animated knob that slides with easing
/// - Animated glow on active state
/// - Optional label
///
/// # Example
///
/// ```ignore
/// let mut toggle = Toggle::new()
///     .label("Dark mode")
///     .on_change(|enabled| println!("Enabled: {}", enabled));
///
/// toggle.animator_mut().enter();
///
/// // In update:
/// toggle.tick();
///
/// // In paint:
/// toggle.paint(bounds, &mut scene, &mut text_system);
///
/// // In event handling:
/// toggle.event(&event, bounds);
/// ```
pub struct Toggle {
    enabled: bool,
    label: Option<String>,
    animator: HudAnimator,

    // State
    hovered: bool,

    // Animation
    toggle_progress: f32,

    // Styling
    track_width: f32,
    track_height: f32,
    knob_size: f32,
    font_size: f32,
    gap: f32,
    border_width: f32,

    // Callbacks
    on_change: Option<OnChange>,
}

impl Toggle {
    /// Create a new toggle.
    pub fn new() -> Self {
        Self {
            enabled: false,
            label: None,
            animator: HudAnimator::new(),
            hovered: false,
            toggle_progress: 0.0,
            track_width: 44.0,
            track_height: 22.0,
            knob_size: 16.0,
            font_size: 14.0,
            gap: 10.0,
            border_width: 1.5,
            on_change: None,
        }
    }

    /// Set the initial enabled state.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.toggle_progress = if enabled { 1.0 } else { 0.0 };
        self
    }

    /// Set the label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the track width.
    pub fn track_width(mut self, width: f32) -> Self {
        self.track_width = width;
        self
    }

    /// Set the track height.
    pub fn track_height(mut self, height: f32) -> Self {
        self.track_height = height;
        self
    }

    /// Set the font size for the label.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the gap between toggle and label.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Set the on_change callback.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Get the current enabled state.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the enabled state programmatically.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
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

        // Animate toggle progress
        let target = if self.enabled { 1.0 } else { 0.0 };
        let speed = 0.12;
        if self.toggle_progress < target {
            self.toggle_progress = (self.toggle_progress + speed).min(target);
        } else if self.toggle_progress > target {
            self.toggle_progress = (self.toggle_progress - speed).max(target);
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self, text_system: &mut TextSystem) -> Size {
        let mut width = self.track_width;
        let mut height = self.track_height;

        if let Some(label) = &self.label {
            let text_size = text_system.measure_size(label, self.font_size, None);
            width += self.gap + text_size.width;
            height = height.max(text_size.height);
        }

        Size::new(width, height)
    }

    /// Paint the toggle.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Calculate track bounds (vertically centered)
        let track_x = bounds.origin.x;
        let track_y = bounds.origin.y + (bounds.size.height - self.track_height) / 2.0;
        let track_bounds = Bounds::new(track_x, track_y, self.track_width, self.track_height);

        let eased = easing::ease_out_expo(self.toggle_progress);

        // Draw track background
        let track_bg_alpha = if self.enabled {
            0.15 * eased
        } else if self.hovered {
            0.05
        } else {
            0.02
        };
        scene.draw_quad(
            wgpui::Quad::new(track_bounds)
                .with_background(Hsla::new(0.0, 0.0, 1.0, track_bg_alpha * progress))
                .with_uniform_radius(self.track_height / 2.0)
        );

        // Draw track border
        let border_color = if self.enabled {
            hud::FRAME_BRIGHT
        } else if self.hovered {
            hud::FRAME_NORMAL
        } else {
            hud::FRAME_DIM
        };
        scene.draw_quad(
            wgpui::Quad::new(track_bounds)
                .with_border(Hsla::new(
                    border_color.h,
                    border_color.s,
                    border_color.l,
                    border_color.a * progress,
                ), self.border_width)
                .with_uniform_radius(self.track_height / 2.0)
        );

        // Calculate knob position
        let knob_padding = (self.track_height - self.knob_size) / 2.0;
        let knob_min_x = track_x + knob_padding;
        let knob_max_x = track_x + self.track_width - knob_padding - self.knob_size;
        let knob_x = knob_min_x + (knob_max_x - knob_min_x) * eased;
        let knob_y = track_y + knob_padding;

        let knob_bounds = Bounds::new(knob_x, knob_y, self.knob_size, self.knob_size);

        // Draw knob
        let knob_color = if self.enabled {
            hud::FRAME_BRIGHT
        } else {
            hud::FRAME_NORMAL
        };
        scene.draw_quad(
            wgpui::Quad::new(knob_bounds)
                .with_background(Hsla::new(
                    knob_color.h,
                    knob_color.s,
                    knob_color.l,
                    knob_color.a * progress,
                ))
                .with_uniform_radius(self.knob_size / 2.0)
        );

        // Draw label
        if let Some(label) = &self.label {
            let text_x = bounds.origin.x + self.track_width + self.gap;
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
                    self.enabled = !self.enabled;
                    if let Some(on_change) = &mut self.on_change {
                        on_change(self.enabled);
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

impl Default for Toggle {
    fn default() -> Self {
        Self::new()
    }
}
