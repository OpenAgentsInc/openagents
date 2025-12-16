//! TextSequence - character-by-character text reveal animation.

use wgpui::{Hsla, Point, Scene, TextSystem};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Animated text that reveals character by character.
///
/// As the animation progresses, more characters become visible,
/// creating a typewriter-like effect.
///
/// # Example
///
/// ```ignore
/// let mut text = TextSequence::new("SYSTEM ONLINE")
///     .font_size(16.0)
///     .color(theme::hud::TEXT);
///
/// text.animator_mut().enter();
///
/// // In update:
/// text.tick();
///
/// // In paint:
/// text.paint(position, &mut scene, &mut text_system);
/// ```
pub struct TextSequence {
    text: String,
    animator: HudAnimator,
    font_size: f32,
    color: Hsla,
    /// Whether to show a cursor at the end during animation.
    show_cursor: bool,
    /// Cursor blink state (for entered state).
    cursor_visible: bool,
    cursor_timer: u32,
}

impl TextSequence {
    /// Create a new text sequence animation.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            animator: HudAnimator::new(),
            font_size: 14.0,
            color: hud::TEXT,
            show_cursor: true,
            cursor_visible: true,
            cursor_timer: 0,
        }
    }

    /// Set the text content.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Enable/disable cursor display.
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
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

    /// Update the text content.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Tick the animation.
    pub fn tick(&mut self) -> bool {
        let animating = self.animator.tick();

        // Blink cursor when fully entered
        if self.animator.state().is_entered() && self.show_cursor {
            self.cursor_timer += 1;
            if self.cursor_timer >= 30 {
                // ~500ms at 60fps
                self.cursor_timer = 0;
                self.cursor_visible = !self.cursor_visible;
            }
        } else {
            self.cursor_visible = true;
            self.cursor_timer = 0;
        }

        animating
    }

    /// Paint the animated text.
    pub fn paint(&self, origin: Point, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let char_count = self.text.chars().count();
        if char_count == 0 {
            return;
        }

        // Calculate how many characters to show
        let visible_chars = ((char_count as f32) * progress).ceil() as usize;
        let visible_text: String = self.text.chars().take(visible_chars).collect();

        // Draw the visible portion
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a);
        let text_run = text_system.layout(&visible_text, origin, self.font_size, color);
        scene.draw_text(text_run);

        // Draw cursor if enabled and animating or blinking
        if self.show_cursor
            && (self.animator.state().is_animating()
                || (self.animator.state().is_entered() && self.cursor_visible))
        {
            // Calculate cursor position
            let cursor_x = origin.x + text_system.measure(&visible_text, self.font_size);
            let cursor_color =
                Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * 0.8);
            let cursor = text_system.layout(
                "_",
                Point::new(cursor_x, origin.y),
                self.font_size,
                cursor_color,
            );
            scene.draw_text(cursor);
        }
    }
}
