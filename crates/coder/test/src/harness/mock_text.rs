//! Mock text system for deterministic text measurement.
//!
//! This provides text measurement without requiring actual font loading,
//! making tests fast and deterministic.

use wgpui::scene::TextRun;
use wgpui::text::FontStyle;
use wgpui::text::TextSystem as WgpuTextSystem;
use wgpui::{Point, Size};

/// Mock text system that provides deterministic text measurement.
///
/// Uses fixed-width character metrics (monospace assumption) for
/// predictable, reproducible test results.
pub struct MockTextSystem {
    /// Width of each character in logical pixels.
    char_width: f32,
    /// Height of each line in logical pixels.
    line_height: f32,
    /// Scale factor (affects measurements).
    scale_factor: f32,
    /// Total layout calls recorded (useful for determinism checks).
    layout_calls: usize,
}

impl MockTextSystem {
    /// Create a new mock text system with default metrics.
    ///
    /// Default: 8px char width, 16px line height.
    pub fn new() -> Self {
        Self {
            char_width: 8.0,
            line_height: 16.0,
            scale_factor: 1.0,
            layout_calls: 0,
        }
    }

    /// Create with custom character metrics.
    pub fn with_metrics(char_width: f32, line_height: f32) -> Self {
        Self {
            char_width,
            line_height,
            scale_factor: 1.0,
            layout_calls: 0,
        }
    }

    /// Set the scale factor.
    pub fn set_scale_factor(&mut self, factor: f32) {
        self.scale_factor = factor;
    }

    /// Get the character width.
    pub fn char_width(&self) -> f32 {
        self.char_width * self.scale_factor
    }

    /// Get the line height.
    pub fn line_height(&self) -> f32 {
        self.line_height * self.scale_factor
    }

    /// Measure the size of a text string.
    pub fn measure(&self, text: &str) -> Size {
        let lines: Vec<&str> = text.lines().collect();
        let line_count = lines.len().max(1);

        let max_width = lines
            .iter()
            .map(|line| self.measure_line(line))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let height = line_count as f32 * self.line_height();

        Size::new(max_width, height)
    }

    /// Measure the width of a single line of text.
    pub fn measure_line(&self, line: &str) -> f32 {
        // Count graphemes for better Unicode handling
        let char_count = line.chars().count();
        // Measure is deterministic and proportional to characters.
        char_count as f32 * self.char_width()
    }

    /// Get the position of a character at the given index.
    pub fn char_position(&self, text: &str, char_index: usize) -> Point {
        let mut line = 0;
        let mut col = 0;

        for (i, c) in text.chars().enumerate() {
            if i == char_index {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }

        Point::new(
            col as f32 * self.char_width(),
            line as f32 * self.line_height(),
        )
    }

    /// Get the character index at the given position.
    pub fn char_index_at(&self, text: &str, position: Point) -> usize {
        let target_line = (position.y / self.line_height()).floor() as usize;
        let target_col = (position.x / self.char_width()).round() as usize;

        let mut current_line = 0;
        let mut current_col = 0;

        for (i, c) in text.chars().enumerate() {
            if current_line == target_line && current_col >= target_col {
                return i;
            }

            if c == '\n' {
                if current_line == target_line {
                    return i; // Clicked past end of line
                }
                current_line += 1;
                current_col = 0;
            } else {
                current_col += 1;
            }
        }

        text.chars().count() // Return end of string
    }

    /// Calculate the bounds of a text selection.
    pub fn selection_bounds(&self, text: &str, start: usize, end: usize) -> Vec<wgpui::Bounds> {
        let start_pos = self.char_position(text, start);
        let end_pos = self.char_position(text, end);

        let start_line = (start_pos.y / self.line_height()) as usize;
        let end_line = (end_pos.y / self.line_height()) as usize;

        let mut bounds = Vec::new();

        if start_line == end_line {
            // Single line selection
            bounds.push(wgpui::Bounds::new(
                start_pos.x,
                start_pos.y,
                end_pos.x - start_pos.x,
                self.line_height(),
            ));
        } else {
            // Multi-line selection
            // First line: from start to end of line
            let first_line_text = text.lines().nth(start_line).unwrap_or("");
            let first_line_width = self.measure_line(first_line_text);
            bounds.push(wgpui::Bounds::new(
                start_pos.x,
                start_pos.y,
                first_line_width - start_pos.x,
                self.line_height(),
            ));

            // Middle lines: full width
            for line_idx in (start_line + 1)..end_line {
                let line_text = text.lines().nth(line_idx).unwrap_or("");
                let line_width = self.measure_line(line_text);
                bounds.push(wgpui::Bounds::new(
                    0.0,
                    line_idx as f32 * self.line_height(),
                    line_width,
                    self.line_height(),
                ));
            }

            // Last line: from start to cursor
            if end_line > start_line {
                bounds.push(wgpui::Bounds::new(
                    0.0,
                    end_pos.y,
                    end_pos.x,
                    self.line_height(),
                ));
            }
        }

        bounds
    }

    /// Record a layout call (for deterministic render assertions).
    pub fn record_layout(&mut self) {
        self.layout_calls += 1;
    }

    /// Get the total layout calls recorded.
    pub fn layout_calls(&self) -> usize {
        self.layout_calls
    }
}

impl TextSystem for MockTextSystem {
    fn measure_size(&mut self, text: &str, font_size: f32, _font_style: Option<FontStyle>) -> Size {
        // scale width by font size relative to default 16px
        let scale = font_size / 16.0;
        let mut clone = self.clone();
        clone.set_scale_factor(self.scale_factor * scale);
        clone.record_layout();
        clone.measure(text)
    }

    fn layout_styled(
        &mut self,
        text: &str,
        origin: Point,
        font_size: f32,
        _color: Color,
        _style: FontStyle,
    ) -> TextRun {
        let size = self.measure_size(text, font_size, None);
        TextRun {
            text: text.to_string(),
            origin,
            size,
        }
    }
}

impl Default for MockTextSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_single_line() {
        let text = MockTextSystem::new();
        let size = text.measure("Hello");

        assert_eq!(size.width, 5.0 * 8.0); // 5 chars * 8px
        assert_eq!(size.height, 16.0); // 1 line * 16px
    }

    #[test]
    fn test_measure_multi_line() {
        let text = MockTextSystem::new();
        let size = text.measure("Hello\nWorld!");

        assert_eq!(size.width, 6.0 * 8.0); // "World!" is longer (6 chars)
        assert_eq!(size.height, 2.0 * 16.0); // 2 lines
    }

    #[test]
    fn test_measure_empty() {
        let text = MockTextSystem::new();
        let size = text.measure("");

        assert_eq!(size.width, 0.0);
        assert_eq!(size.height, 16.0); // Still 1 line height
    }

    #[test]
    fn test_char_position() {
        let text = MockTextSystem::new();

        // First character
        let pos = text.char_position("Hello", 0);
        assert_eq!(pos.x, 0.0);
        assert_eq!(pos.y, 0.0);

        // Middle character
        let pos = text.char_position("Hello", 2);
        assert_eq!(pos.x, 16.0); // 2 * 8px
        assert_eq!(pos.y, 0.0);

        // Second line
        let pos = text.char_position("Hello\nWorld", 8);
        assert_eq!(pos.x, 16.0); // 2 chars into second line ("Wo")
        assert_eq!(pos.y, 16.0); // Second line
    }

    #[test]
    fn test_char_index_at() {
        let text = MockTextSystem::new();

        // Beginning
        let idx = text.char_index_at("Hello", Point::new(0.0, 0.0));
        assert_eq!(idx, 0);

        // Middle of text (20px / 8px = 2.5 chars, rounds to 3)
        let idx = text.char_index_at("Hello", Point::new(20.0, 0.0));
        assert_eq!(idx, 3);

        // End of text
        let idx = text.char_index_at("Hello", Point::new(100.0, 0.0));
        assert_eq!(idx, 5);
    }

    #[test]
    fn test_scale_factor() {
        let mut text = MockTextSystem::new();
        text.set_scale_factor(2.0);

        assert_eq!(text.char_width(), 16.0); // 8 * 2
        assert_eq!(text.line_height(), 32.0); // 16 * 2

        let size = text.measure("Hi");
        assert_eq!(size.width, 32.0); // 2 * 16
        assert_eq!(size.height, 32.0); // 1 * 32
    }

    #[test]
    fn test_custom_metrics() {
        let text = MockTextSystem::with_metrics(10.0, 20.0);

        assert_eq!(text.char_width(), 10.0);
        assert_eq!(text.line_height(), 20.0);

        let size = text.measure("ABC");
        assert_eq!(size.width, 30.0);
        assert_eq!(size.height, 20.0);
    }
}
