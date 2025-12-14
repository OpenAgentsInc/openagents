//! TextInput - sci-fi styled text input with animated underline.

use wgpui::{Bounds, Hsla, InputEvent, Key, MouseButton, NamedKey, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// Callback for value change.
pub type OnChange = Box<dyn FnMut(&str)>;

/// Sci-fi styled text input with animated underline.
///
/// Features:
/// - Animated underline that expands from center on focus
/// - Blinking cursor
/// - Placeholder text support
/// - Character-by-character input
///
/// # Example
///
/// ```ignore
/// let mut input = TextInput::new()
///     .placeholder("Enter name...")
///     .on_change(|value| println!("Value: {}", value));
///
/// input.animator_mut().enter();
///
/// // In update:
/// input.tick();
///
/// // In paint:
/// input.paint(bounds, &mut scene, &mut text_system);
///
/// // In event handling:
/// input.event(&event, bounds);
/// ```
pub struct TextInput {
    value: String,
    placeholder: String,
    animator: HudAnimator,

    // State
    focused: bool,
    cursor_pos: usize,
    cursor_blink_time: f32,
    selection_start: Option<usize>,

    // Animation
    focus_progress: f32,

    // Styling
    font_size: f32,
    padding: (f32, f32),
    underline_width: f32,

    // Callbacks
    on_change: Option<OnChange>,
}

impl TextInput {
    /// Create a new text input.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            animator: HudAnimator::new(),
            focused: false,
            cursor_pos: 0,
            cursor_blink_time: 0.0,
            selection_start: None,
            focus_progress: 0.0,
            font_size: 14.0,
            padding: (12.0, 8.0),
            underline_width: 2.0,
            on_change: None,
        }
    }

    /// Set the initial value.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_pos = self.value.len();
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
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

    /// Set the underline width.
    pub fn underline_width(mut self, width: f32) -> Self {
        self.underline_width = width;
        self
    }

    /// Set the on_change callback.
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Get the current value.
    pub fn get_value(&self) -> &str {
        &self.value
    }

    /// Set the value programmatically.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_pos = self.cursor_pos.min(self.value.len());
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Check if the input is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.cursor_blink_time = 0.0;
        }
    }

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();

        // Animate focus progress
        let target = if self.focused { 1.0 } else { 0.0 };
        let speed = 0.15;
        if self.focus_progress < target {
            self.focus_progress = (self.focus_progress + speed).min(target);
        } else if self.focus_progress > target {
            self.focus_progress = (self.focus_progress - speed).max(target);
        }

        // Blink cursor
        if self.focused {
            self.cursor_blink_time += 1.0 / 60.0; // Assume 60fps
            if self.cursor_blink_time > 1.0 {
                self.cursor_blink_time -= 1.0;
            }
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self, min_width: f32) -> Size {
        let height = self.font_size + self.padding.1 * 2.0 + self.underline_width;
        Size::new(min_width.max(200.0), height)
    }

    /// Paint the text input.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let content_y = bounds.origin.y + self.padding.1;
        let underline_y = bounds.origin.y + bounds.size.height - self.underline_width;

        // Draw background on focus
        if self.focus_progress > 0.0 {
            let bg_alpha = 0.03 * self.focus_progress * progress;
            scene.draw_quad(
                wgpui::Quad::new(bounds)
                    .with_background(Hsla::new(0.0, 0.0, 1.0, bg_alpha))
            );
        }

        // Draw base underline (dim)
        let base_color = Hsla::new(
            hud::FRAME_DIM.h,
            hud::FRAME_DIM.s,
            hud::FRAME_DIM.l,
            hud::FRAME_DIM.a * progress,
        );
        scene.draw_quad(
            wgpui::Quad::new(Bounds::new(
                bounds.origin.x, underline_y,
                bounds.size.width, self.underline_width,
            ))
            .with_background(base_color)
        );

        // Draw focus underline (expands from center)
        if self.focus_progress > 0.0 {
            let eased = easing::ease_out_expo(self.focus_progress);
            let focus_width = bounds.size.width * eased;
            let focus_x = bounds.origin.x + (bounds.size.width - focus_width) / 2.0;

            let focus_color = Hsla::new(
                hud::FRAME_BRIGHT.h,
                hud::FRAME_BRIGHT.s,
                hud::FRAME_BRIGHT.l,
                hud::FRAME_BRIGHT.a * progress,
            );
            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    focus_x, underline_y,
                    focus_width, self.underline_width,
                ))
                .with_background(focus_color)
            );
        }

        // Draw text or placeholder
        let text_x = bounds.origin.x + self.padding.0;

        if self.value.is_empty() && !self.placeholder.is_empty() {
            // Draw placeholder
            let placeholder_color = Hsla::new(
                hud::TEXT_MUTED.h,
                hud::TEXT_MUTED.s,
                hud::TEXT_MUTED.l,
                hud::TEXT_MUTED.a * 0.5 * progress,
            );
            let text_run = text_system.layout(
                &self.placeholder,
                Point::new(text_x, content_y),
                self.font_size,
                placeholder_color,
            );
            scene.draw_text(text_run);
        } else {
            // Draw value
            let text_color = Hsla::new(
                hud::TEXT.h,
                hud::TEXT.s,
                hud::TEXT.l,
                hud::TEXT.a * progress,
            );
            let text_run = text_system.layout(
                &self.value,
                Point::new(text_x, content_y),
                self.font_size,
                text_color,
            );
            scene.draw_text(text_run);
        }

        // Draw cursor
        if self.focused && self.cursor_blink_time < 0.5 {
            let text_before_cursor = &self.value[..self.cursor_pos];
            let cursor_offset = text_system.measure_size(text_before_cursor, self.font_size, None).width;
            let cursor_x = text_x + cursor_offset;

            let cursor_color = Hsla::new(
                hud::FRAME_BRIGHT.h,
                hud::FRAME_BRIGHT.s,
                hud::FRAME_BRIGHT.l,
                hud::FRAME_BRIGHT.a * progress,
            );
            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    cursor_x, content_y,
                    2.0, self.font_size,
                ))
                .with_background(cursor_color)
            );
        }
    }

    /// Handle an input event.
    ///
    /// Returns `true` if the event was handled.
    pub fn event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        match event {
            InputEvent::MouseDown { position, button, .. } => {
                if *button == MouseButton::Left {
                    let was_focused = self.focused;
                    self.focused = bounds.contains(*position);

                    if self.focused {
                        self.cursor_blink_time = 0.0;
                        // TODO: Calculate cursor position from click
                        self.cursor_pos = self.value.len();
                    }

                    was_focused != self.focused
                } else {
                    false
                }
            }

            InputEvent::KeyDown { key, .. } if self.focused => {
                match key {
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                            self.cursor_blink_time = 0.0;
                        }
                        true
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.cursor_pos < self.value.len() {
                            self.cursor_pos += 1;
                            self.cursor_blink_time = 0.0;
                        }
                        true
                    }
                    Key::Named(NamedKey::Home) => {
                        self.cursor_pos = 0;
                        self.cursor_blink_time = 0.0;
                        true
                    }
                    Key::Named(NamedKey::End) => {
                        self.cursor_pos = self.value.len();
                        self.cursor_blink_time = 0.0;
                        true
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                            self.value.remove(self.cursor_pos);
                            self.cursor_blink_time = 0.0;
                            if let Some(on_change) = &mut self.on_change {
                                on_change(&self.value);
                            }
                        }
                        true
                    }
                    Key::Named(NamedKey::Delete) => {
                        if self.cursor_pos < self.value.len() {
                            self.value.remove(self.cursor_pos);
                            self.cursor_blink_time = 0.0;
                            if let Some(on_change) = &mut self.on_change {
                                on_change(&self.value);
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }

            InputEvent::TextInput { text } if self.focused => {
                // Insert text at cursor position
                for ch in text.chars() {
                    if ch >= ' ' && ch != '\x7f' {
                        self.value.insert(self.cursor_pos, ch);
                        self.cursor_pos += 1;
                    }
                }
                self.cursor_blink_time = 0.0;
                if let Some(on_change) = &mut self.on_change {
                    on_change(&self.value);
                }
                true
            }

            _ => false,
        }
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}
