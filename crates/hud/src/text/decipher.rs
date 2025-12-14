//! TextDecipher - scramble/decipher text animation effect.

use wgpui::{Hsla, Point, Scene, TextSystem};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Characters to use for scrambling effect.
const SCRAMBLE_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*";

/// Animated text that deciphers from random characters to final text.
///
/// Creates a "hacking" effect where random characters gradually
/// resolve into the target text.
///
/// # Example
///
/// ```ignore
/// let mut text = TextDecipher::new("ACCESS GRANTED")
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
pub struct TextDecipher {
    text: String,
    animator: HudAnimator,
    font_size: f32,
    color: Hsla,
    /// Random seed for consistent scrambling per instance.
    seed: u32,
    /// Frame counter for character cycling.
    frame: u32,
    /// How often to change scramble characters (in frames).
    scramble_speed: u32,
}

impl TextDecipher {
    /// Create a new text decipher animation.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            animator: HudAnimator::new(),
            font_size: 14.0,
            color: hud::TEXT,
            seed: 12345,
            frame: 0,
            scramble_speed: 3,
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

    /// Set scramble speed (lower = faster character cycling).
    pub fn scramble_speed(mut self, speed: u32) -> Self {
        self.scramble_speed = speed.max(1);
        self
    }

    /// Set random seed for reproducible scrambling.
    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = seed;
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
        self.frame = self.frame.wrapping_add(1);
        self.animator.tick()
    }

    /// Simple pseudo-random number generator.
    fn random(&self, index: usize) -> usize {
        let x = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        let x = x.wrapping_mul((index as u32).wrapping_add(1));
        let x = x.wrapping_add(self.frame / self.scramble_speed);
        (x as usize) % SCRAMBLE_CHARS.len()
    }

    /// Get a scrambled character for the given index.
    fn scramble_char(&self, index: usize) -> char {
        let rand_index = self.random(index);
        SCRAMBLE_CHARS.chars().nth(rand_index).unwrap_or('?')
    }

    /// Paint the animated text.
    pub fn paint(&self, origin: Point, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let chars: Vec<char> = self.text.chars().collect();
        let char_count = chars.len();
        if char_count == 0 {
            return;
        }

        // Calculate how many characters are "deciphered"
        let deciphered_count = ((char_count as f32) * progress).floor() as usize;

        // Build the display string
        let display: String = chars
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                if i < deciphered_count {
                    // This character is fully deciphered
                    c
                } else if c.is_whitespace() {
                    // Keep spaces as spaces
                    c
                } else {
                    // Scramble this character
                    self.scramble_char(i)
                }
            })
            .collect();

        // Draw the text
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a);
        let text_run = text_system.layout(&display, origin, self.font_size, color);
        scene.draw_text(text_run);
    }
}
