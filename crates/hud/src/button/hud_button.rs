//! HudButton - sci-fi styled button with animated frame.

use wgpui::{Bounds, Hsla, InputEvent, MouseButton, Point, Scene, TextSystem};

use crate::animator::HudAnimator;
use crate::frame::FrameCorners;
use crate::theme::hud;

/// Callback for button click.
pub type OnClick = Box<dyn FnMut()>;

/// Sci-fi styled button with hover/active animations.
///
/// The button uses a FrameCorners for its border and animates
/// both the frame and content.
///
/// # Example
///
/// ```ignore
/// let mut button = HudButton::new("CONNECT")
///     .font_size(14.0)
///     .on_click(|| println!("Clicked!"));
///
/// button.animator_mut().enter();
///
/// // In update:
/// button.tick();
///
/// // In paint:
/// button.paint(bounds, &mut scene, &mut text_system);
///
/// // In event handling:
/// button.event(&event, bounds);
/// ```
pub struct HudButton {
    label: String,
    animator: HudAnimator,
    corner_length: f32,

    // State
    hovered: bool,
    pressed: bool,

    // Styling
    font_size: f32,
    padding: (f32, f32), // (horizontal, vertical)

    // Callbacks
    on_click: Option<OnClick>,
}

impl HudButton {
    /// Create a new button with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            animator: HudAnimator::new(),
            corner_length: 12.0,
            hovered: false,
            pressed: false,
            font_size: 14.0,
            padding: (20.0, 10.0),
            on_click: None,
        }
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the padding (horizontal, vertical).
    pub fn padding(mut self, h: f32, v: f32) -> Self {
        self.padding = (h, v);
        self
    }

    /// Set the corner length on the frame.
    pub fn corner_length(mut self, len: f32) -> Self {
        self.corner_length = len;
        self
    }

    /// Set the on_click callback.
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Get the button label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Set the button label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Check if the button is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Check if the button is pressed.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Tick all animations.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Calculate the preferred size for this button.
    pub fn preferred_size(&self) -> (f32, f32) {
        let text_width = self.label.len() as f32 * self.font_size * 0.6;
        let width = text_width + self.padding.0 * 2.0;
        let height = self.font_size + self.padding.1 * 2.0;
        (width, height)
    }

    /// Paint the button.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Calculate colors based on state
        let (frame_color, text_color, bg_alpha) = if self.pressed {
            (hud::FRAME_DIM, hud::TEXT_MUTED, 0.15)
        } else if self.hovered {
            (hud::FRAME_BRIGHT, hud::FRAME_BRIGHT, 0.08) // Subtle hover background
        } else {
            (hud::FRAME_NORMAL, hud::TEXT, 0.0)
        };

        // Draw hover/pressed background
        if bg_alpha > 0.0 {
            scene.draw_quad(wgpui::Quad::new(bounds).with_background(Hsla::new(
                0.0,
                0.0,
                1.0,
                bg_alpha * progress,
            )));
        }

        // Draw frame with state-based color
        let mut frame = FrameCorners::new()
            .corner_length(self.corner_length)
            .line_width(1.0)
            .color(Hsla::new(
                frame_color.h,
                frame_color.s,
                frame_color.l,
                frame_color.a * progress,
            ));
        frame.animator_mut().set_entered();
        frame.paint(bounds, scene);

        // Draw label - measure using text system for accurate centering
        let text_size = text_system.measure_size(&self.label, self.font_size, None);

        // Center horizontally and vertically within bounds
        let text_x = bounds.origin.x + (bounds.size.width - text_size.width) / 2.0;
        let text_y = bounds.origin.y + (bounds.size.height - text_size.height) / 2.0;

        let final_text_color = Hsla::new(
            text_color.h,
            text_color.s,
            text_color.l,
            text_color.a * progress,
        );
        let text_run = text_system.layout(
            &self.label,
            Point::new(text_x, text_y),
            self.font_size,
            final_text_color,
        );
        scene.draw_text(text_run);
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

            InputEvent::MouseDown {
                position, button, ..
            } => {
                if *button == MouseButton::Left && bounds.contains(*position) {
                    self.pressed = true;
                    true
                } else {
                    false
                }
            }

            InputEvent::MouseUp {
                position, button, ..
            } => {
                if *button == MouseButton::Left && self.pressed {
                    self.pressed = false;

                    // Fire click if released over button
                    if bounds.contains(*position) {
                        if let Some(on_click) = &mut self.on_click {
                            on_click();
                        }
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
