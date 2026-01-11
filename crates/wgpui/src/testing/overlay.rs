//! Input overlay for visualizing test interactions.
//!
//! Renders cursor position, click ripples, and key presses on top of
//! the component under test.

use crate::animation::Easing;
use crate::components::{Component, ComponentId, EventResult};
use crate::components::{EventContext, PaintContext};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of concurrent click ripples.
const MAX_RIPPLES: usize = 5;

/// Maximum number of visible key presses.
const MAX_KEYS: usize = 5;

/// Duration of a click ripple animation.
const RIPPLE_DURATION: Duration = Duration::from_millis(400);

/// Duration before a key press fades out.
const KEY_DISPLAY_DURATION: Duration = Duration::from_millis(800);

/// A click ripple animation.
#[derive(Clone, Debug)]
pub struct ClickRipple {
    /// Center position of the ripple.
    pub position: Point,
    /// When the ripple started.
    pub started: Instant,
    /// Mouse button that was clicked.
    pub button: MouseButton,
}

impl ClickRipple {
    /// Create a new ripple at a position.
    pub fn new(position: Point, button: MouseButton) -> Self {
        Self {
            position,
            started: Instant::now(),
            button,
        }
    }

    /// Get the progress of the animation (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let elapsed = self.started.elapsed().as_secs_f32();
        let duration = RIPPLE_DURATION.as_secs_f32();
        (elapsed / duration).min(1.0)
    }

    /// Check if the ripple animation is complete.
    pub fn is_complete(&self) -> bool {
        self.started.elapsed() >= RIPPLE_DURATION
    }

    /// Get the current radius based on animation progress.
    pub fn current_radius(&self, max_radius: f32) -> f32 {
        let t = Easing::EaseOutQuad.apply(self.progress());
        max_radius * t
    }

    /// Get the current opacity based on animation progress.
    pub fn current_opacity(&self) -> f32 {
        let t = self.progress();
        // Fade out as the ripple expands
        1.0 - Easing::EaseInQuad.apply(t)
    }
}

/// A key press display entry.
#[derive(Clone, Debug)]
pub struct KeyDisplay {
    /// The key that was pressed.
    pub key_text: String,
    /// When the key was pressed.
    pub pressed: Instant,
}

impl KeyDisplay {
    /// Create a new key display.
    pub fn new(key_text: impl Into<String>) -> Self {
        Self {
            key_text: key_text.into(),
            pressed: Instant::now(),
        }
    }

    /// Get the time since the key was pressed.
    pub fn age(&self) -> Duration {
        self.pressed.elapsed()
    }

    /// Check if the key display should be removed.
    pub fn is_expired(&self) -> bool {
        self.age() >= KEY_DISPLAY_DURATION
    }

    /// Get the current opacity based on age.
    pub fn current_opacity(&self) -> f32 {
        let t = self.age().as_secs_f32() / KEY_DISPLAY_DURATION.as_secs_f32();
        if t > 0.5 {
            // Start fading after halfway
            1.0 - (t - 0.5) * 2.0
        } else {
            1.0
        }
    }
}

/// Position for the key display stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyDisplayPosition {
    /// Bottom-left corner.
    #[default]
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
}

/// Input overlay component for test visualization.
pub struct InputOverlay {
    id: Option<ComponentId>,
    /// Current mouse position.
    mouse_position: Point,
    /// Active click ripples.
    ripples: VecDeque<ClickRipple>,
    /// Recent key presses.
    key_presses: VecDeque<KeyDisplay>,
    /// Size of the cursor crosshair.
    cursor_size: f32,
    /// Maximum radius of click ripples.
    ripple_max_radius: f32,
    /// Position for key display.
    key_position: KeyDisplayPosition,
    /// Whether the overlay is visible.
    visible: bool,
}

impl Default for InputOverlay {
    fn default() -> Self {
        Self {
            id: None,
            mouse_position: Point::new(0.0, 0.0),
            ripples: VecDeque::with_capacity(MAX_RIPPLES),
            key_presses: VecDeque::with_capacity(MAX_KEYS),
            cursor_size: 16.0,
            ripple_max_radius: 30.0,
            key_position: KeyDisplayPosition::BottomLeft,
            visible: true,
        }
    }
}

impl InputOverlay {
    /// Create a new input overlay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the component ID.
    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the cursor size.
    pub fn with_cursor_size(mut self, size: f32) -> Self {
        self.cursor_size = size;
        self
    }

    /// Set the ripple max radius.
    pub fn with_ripple_radius(mut self, radius: f32) -> Self {
        self.ripple_max_radius = radius;
        self
    }

    /// Set the key display position.
    pub fn with_key_position(mut self, position: KeyDisplayPosition) -> Self {
        self.key_position = position;
        self
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Get the current mouse position.
    pub fn mouse_position(&self) -> Point {
        self.mouse_position
    }

    /// Update from an input event.
    pub fn observe_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.mouse_position = Point::new(*x, *y);
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                self.mouse_position = Point::new(*x, *y);
                self.add_ripple(Point::new(*x, *y), *button);
            }
            InputEvent::KeyDown { key, modifiers } => {
                let key_text = format_key_display(key, modifiers);
                self.add_key_press(key_text);
            }
            _ => {}
        }
    }

    /// Add a click ripple.
    fn add_ripple(&mut self, position: Point, button: MouseButton) {
        if self.ripples.len() >= MAX_RIPPLES {
            self.ripples.pop_front();
        }
        self.ripples.push_back(ClickRipple::new(position, button));
    }

    /// Add a key press display.
    fn add_key_press(&mut self, key_text: String) {
        if self.key_presses.len() >= MAX_KEYS {
            self.key_presses.pop_front();
        }
        self.key_presses.push_back(KeyDisplay::new(key_text));
    }

    /// Clean up expired animations.
    pub fn cleanup(&mut self) {
        self.ripples.retain(|r| !r.is_complete());
        self.key_presses.retain(|k| !k.is_expired());
    }

    /// Paint the cursor crosshair.
    fn paint_cursor(&self, cx: &mut PaintContext) {
        let x = self.mouse_position.x;
        let y = self.mouse_position.y;
        let size = self.cursor_size;
        let half = size / 2.0;
        let color = theme::accent::PRIMARY;

        // Horizontal line
        cx.scene
            .draw_quad(Quad::new(Bounds::new(x - half, y - 0.5, size, 1.0)).with_background(color));

        // Vertical line
        cx.scene
            .draw_quad(Quad::new(Bounds::new(x - 0.5, y - half, 1.0, size)).with_background(color));
    }

    /// Paint click ripples.
    fn paint_ripples(&self, cx: &mut PaintContext) {
        for ripple in &self.ripples {
            let radius = ripple.current_radius(self.ripple_max_radius);
            let opacity = ripple.current_opacity();
            let color = match ripple.button {
                MouseButton::Left => theme::accent::PRIMARY.with_alpha(opacity * 0.5),
                MouseButton::Right => theme::status::WARNING.with_alpha(opacity * 0.5),
                MouseButton::Middle => theme::status::INFO.with_alpha(opacity * 0.5),
            };

            // Draw a circle as a square (no border-radius per AGENTS.md)
            let bounds = Bounds::new(
                ripple.position.x - radius,
                ripple.position.y - radius,
                radius * 2.0,
                radius * 2.0,
            );
            cx.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(color)
                    .with_border(color.with_alpha(opacity * 0.8), 2.0),
            );
        }
    }

    /// Paint key press display.
    fn paint_key_presses(&self, bounds: Bounds, cx: &mut PaintContext) {
        if self.key_presses.is_empty() {
            return;
        }

        let padding = 8.0;
        let key_height = 24.0;
        let key_padding = 4.0;
        let stack_height = self.key_presses.len() as f32 * (key_height + key_padding);

        // Calculate base position based on key_position
        let (base_x, base_y, align_right) = match self.key_position {
            KeyDisplayPosition::BottomLeft => (
                bounds.origin.x + padding,
                bounds.origin.y + bounds.size.height - padding - stack_height,
                false,
            ),
            KeyDisplayPosition::BottomRight => (
                bounds.origin.x + bounds.size.width - padding,
                bounds.origin.y + bounds.size.height - padding - stack_height,
                true,
            ),
            KeyDisplayPosition::TopLeft => {
                (bounds.origin.x + padding, bounds.origin.y + padding, false)
            }
            KeyDisplayPosition::TopRight => (
                bounds.origin.x + bounds.size.width - padding,
                bounds.origin.y + padding,
                true,
            ),
        };

        for (i, key) in self.key_presses.iter().enumerate() {
            let opacity = key.current_opacity();
            let y = base_y + i as f32 * (key_height + key_padding);

            // Measure text width (approximate)
            let text_width = key.key_text.len() as f32 * 8.0 + 16.0;
            let x = if align_right {
                base_x - text_width
            } else {
                base_x
            };

            // Background
            let bg_bounds = Bounds::new(x, y, text_width, key_height);
            cx.scene.draw_quad(
                Quad::new(bg_bounds)
                    .with_background(theme::bg::ELEVATED.with_alpha(0.8 * opacity))
                    .with_border(theme::border::DEFAULT.with_alpha(opacity), 1.0),
            );

            // Text
            let text_run = cx.text.layout(
                &key.key_text,
                Point::new(x + 8.0, y + 5.0),
                theme::font_size::SM,
                theme::text::PRIMARY.with_alpha(opacity),
            );
            cx.scene.draw_text(text_run);
        }
    }
}

impl Component for InputOverlay {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.visible {
            return;
        }

        // Clean up expired animations
        self.cleanup();

        // Paint in order: ripples (back), cursor (front), keys (UI)
        self.paint_ripples(cx);
        self.paint_cursor(cx);
        self.paint_key_presses(bounds, cx);
    }

    fn event(
        &mut self,
        event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        // Observe events but don't consume them
        self.observe_event(event);
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None) // Overlay uses full bounds
    }
}

/// Format a key and modifiers for display.
fn format_key_display(key: &crate::Key, modifiers: &crate::Modifiers) -> String {
    let mut parts = Vec::new();

    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.meta {
        #[cfg(target_os = "macos")]
        parts.push("Cmd");
        #[cfg(not(target_os = "macos"))]
        parts.push("Win");
    }

    let key_text = match key {
        crate::Key::Named(named) => match named {
            crate::NamedKey::Enter => "Enter".to_string(),
            crate::NamedKey::Escape => "Esc".to_string(),
            crate::NamedKey::Backspace => "Backspace".to_string(),
            crate::NamedKey::Delete => "Delete".to_string(),
            crate::NamedKey::Tab => "Tab".to_string(),
            crate::NamedKey::Home => "Home".to_string(),
            crate::NamedKey::End => "End".to_string(),
            crate::NamedKey::ArrowUp => "↑".to_string(),
            crate::NamedKey::ArrowDown => "↓".to_string(),
            crate::NamedKey::ArrowLeft => "←".to_string(),
            crate::NamedKey::ArrowRight => "→".to_string(),
            crate::NamedKey::PageUp => "PgUp".to_string(),
            crate::NamedKey::PageDown => "PgDn".to_string(),
            crate::NamedKey::Unidentified => "?".to_string(),
        },
        crate::Key::Character(c) => {
            if c == " " {
                "Space".to_string()
            } else {
                c.to_uppercase()
            }
        }
    };

    parts.push(&key_text);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_ripple_progress() {
        let ripple = ClickRipple::new(Point::new(100.0, 100.0), MouseButton::Left);
        assert!(ripple.progress() < 0.1); // Just started
        assert!(!ripple.is_complete());
    }

    #[test]
    fn test_key_display_opacity() {
        let key = KeyDisplay::new("Enter");
        assert!(key.current_opacity() > 0.9); // Just pressed, should be visible
        assert!(!key.is_expired());
    }

    #[test]
    fn test_overlay_observe_mouse_move() {
        let mut overlay = InputOverlay::new();
        overlay.observe_event(&InputEvent::MouseMove { x: 50.0, y: 75.0 });
        assert_eq!(overlay.mouse_position().x, 50.0);
        assert_eq!(overlay.mouse_position().y, 75.0);
    }

    #[test]
    fn test_overlay_observe_click() {
        let mut overlay = InputOverlay::new();
        overlay.observe_event(&InputEvent::MouseDown { button: MouseButton::Left, x: 100.0, y: 200.0, modifiers: Modifiers::default() });
        assert_eq!(overlay.ripples.len(), 1);
    }

    #[test]
    fn test_overlay_observe_key() {
        let mut overlay = InputOverlay::new();
        overlay.observe_event(&InputEvent::KeyDown {
            key: crate::Key::Named(crate::NamedKey::Enter),
            modifiers: crate::Modifiers::default(),
        });
        assert_eq!(overlay.key_presses.len(), 1);
        assert_eq!(overlay.key_presses[0].key_text, "Enter");
    }

    #[test]
    fn test_overlay_key_press_stack_caps() {
        let mut overlay = InputOverlay::new();
        for ch in ["a", "b", "c", "d", "e", "f"] {
            overlay.observe_event(&InputEvent::KeyDown {
                key: crate::Key::Character(ch.to_string()),
                modifiers: crate::Modifiers::default(),
            });
        }

        assert_eq!(overlay.key_presses.len(), MAX_KEYS);
        assert_eq!(overlay.key_presses[0].key_text, "B");
        assert_eq!(overlay.key_presses[4].key_text, "F");
    }

    #[test]
    fn test_format_key_display() {
        assert_eq!(
            format_key_display(
                &crate::Key::Named(crate::NamedKey::Enter),
                &crate::Modifiers::default()
            ),
            "Enter"
        );

        assert_eq!(
            format_key_display(
                &crate::Key::Character("s".to_string()),
                &crate::Modifiers {
                    ctrl: true,
                    ..Default::default()
                }
            ),
            "Ctrl+S"
        );
    }
}
