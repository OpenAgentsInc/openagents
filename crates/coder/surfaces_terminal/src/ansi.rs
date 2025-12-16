//! ANSI escape sequence parsing using the vte crate.

use wgpui::Hsla;

/// ANSI text style attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnsiStyle {
    /// Foreground color.
    pub fg: Option<Hsla>,
    /// Background color.
    pub bg: Option<Hsla>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Dim/faint text.
    pub dim: bool,
    /// Strikethrough text.
    pub strikethrough: bool,
    /// Inverse video (swap fg/bg).
    pub inverse: bool,
}

impl AnsiStyle {
    /// Create a new default style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set foreground color.
    pub fn with_fg(mut self, color: Hsla) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set background color.
    pub fn with_bg(mut self, color: Hsla) -> Self {
        self.bg = Some(color);
        self
    }

    /// Set bold.
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Get effective foreground color.
    pub fn effective_fg(&self) -> Hsla {
        let base = self.fg.unwrap_or(DEFAULT_FG);
        if self.inverse {
            self.bg.unwrap_or(DEFAULT_BG)
        } else if self.dim {
            Hsla::new(base.h, base.s, base.l * 0.6, base.a)
        } else {
            base
        }
    }

    /// Get effective background color.
    pub fn effective_bg(&self) -> Hsla {
        if self.inverse {
            self.fg.unwrap_or(DEFAULT_FG)
        } else {
            self.bg.unwrap_or(DEFAULT_BG)
        }
    }
}

/// Default foreground color (light gray).
pub const DEFAULT_FG: Hsla = Hsla::new(0.0, 0.0, 0.85, 1.0);

/// Default background color (transparent).
pub const DEFAULT_BG: Hsla = Hsla::new(0.0, 0.0, 0.0, 0.0);

/// ANSI 16-color palette.
pub mod colors {
    use wgpui::Hsla;

    pub const BLACK: Hsla = Hsla::new(0.0, 0.0, 0.0, 1.0);
    pub const RED: Hsla = Hsla::new(0.0, 0.8, 0.5, 1.0);
    pub const GREEN: Hsla = Hsla::new(120.0 / 360.0, 0.8, 0.4, 1.0);
    pub const YELLOW: Hsla = Hsla::new(60.0 / 360.0, 0.8, 0.5, 1.0);
    pub const BLUE: Hsla = Hsla::new(220.0 / 360.0, 0.8, 0.5, 1.0);
    pub const MAGENTA: Hsla = Hsla::new(300.0 / 360.0, 0.8, 0.5, 1.0);
    pub const CYAN: Hsla = Hsla::new(180.0 / 360.0, 0.8, 0.5, 1.0);
    pub const WHITE: Hsla = Hsla::new(0.0, 0.0, 0.75, 1.0);

    // Bright variants
    pub const BRIGHT_BLACK: Hsla = Hsla::new(0.0, 0.0, 0.4, 1.0);
    pub const BRIGHT_RED: Hsla = Hsla::new(0.0, 0.9, 0.6, 1.0);
    pub const BRIGHT_GREEN: Hsla = Hsla::new(120.0 / 360.0, 0.9, 0.5, 1.0);
    pub const BRIGHT_YELLOW: Hsla = Hsla::new(60.0 / 360.0, 0.9, 0.6, 1.0);
    pub const BRIGHT_BLUE: Hsla = Hsla::new(220.0 / 360.0, 0.9, 0.6, 1.0);
    pub const BRIGHT_MAGENTA: Hsla = Hsla::new(300.0 / 360.0, 0.9, 0.6, 1.0);
    pub const BRIGHT_CYAN: Hsla = Hsla::new(180.0 / 360.0, 0.9, 0.6, 1.0);
    pub const BRIGHT_WHITE: Hsla = Hsla::new(0.0, 0.0, 1.0, 1.0);

    /// Get color by ANSI index (0-15).
    pub fn by_index(index: u8) -> Hsla {
        match index {
            0 => BLACK,
            1 => RED,
            2 => GREEN,
            3 => YELLOW,
            4 => BLUE,
            5 => MAGENTA,
            6 => CYAN,
            7 => WHITE,
            8 => BRIGHT_BLACK,
            9 => BRIGHT_RED,
            10 => BRIGHT_GREEN,
            11 => BRIGHT_YELLOW,
            12 => BRIGHT_BLUE,
            13 => BRIGHT_MAGENTA,
            14 => BRIGHT_CYAN,
            15 => BRIGHT_WHITE,
            // 256-color: 16-231 are 6x6x6 color cube
            16..=231 => {
                let n = index - 16;
                let b = (n % 6) as f32 / 5.0;
                let g = ((n / 6) % 6) as f32 / 5.0;
                let r = (n / 36) as f32 / 5.0;
                // Convert RGB to HSL (simplified)
                let l = (r + g + b) / 3.0;
                Hsla::new(0.0, 0.0, l, 1.0)
            }
            // 232-255 are grayscale
            232..=255 => {
                let gray = (index - 232) as f32 / 23.0;
                Hsla::new(0.0, 0.0, gray * 0.9 + 0.08, 1.0)
            }
        }
    }
}

/// ANSI parser using vte.
pub struct AnsiParser {
    /// Current style.
    current_style: AnsiStyle,
    /// Parsed segments.
    segments: Vec<(String, AnsiStyle)>,
    /// Current text buffer.
    current_text: String,
}

impl AnsiParser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            current_style: AnsiStyle::default(),
            segments: Vec::new(),
            current_text: String::new(),
        }
    }

    /// Parse ANSI text and return styled segments.
    pub fn parse(&mut self, input: &str) -> Vec<(String, AnsiStyle)> {
        self.segments.clear();
        self.current_text.clear();
        self.current_style = AnsiStyle::default();

        let mut parser = vte::Parser::new();
        parser.advance(self, input.as_bytes());

        // Flush remaining text
        if !self.current_text.is_empty() {
            self.segments
                .push((std::mem::take(&mut self.current_text), self.current_style));
        }

        std::mem::take(&mut self.segments)
    }

    /// Reset parser state.
    pub fn reset(&mut self) {
        self.current_style = AnsiStyle::default();
        self.segments.clear();
        self.current_text.clear();
    }

    /// Process SGR (Select Graphic Rendition) parameters.
    fn process_sgr(&mut self, params: &[&[u16]]) {
        let mut i = 0;
        while i < params.len() {
            let param = params[i].first().copied().unwrap_or(0);
            match param {
                0 => self.current_style = AnsiStyle::default(),
                1 => self.current_style.bold = true,
                2 => self.current_style.dim = true,
                3 => self.current_style.italic = true,
                4 => self.current_style.underline = true,
                7 => self.current_style.inverse = true,
                9 => self.current_style.strikethrough = true,
                21 | 22 => {
                    self.current_style.bold = false;
                    self.current_style.dim = false;
                }
                23 => self.current_style.italic = false,
                24 => self.current_style.underline = false,
                27 => self.current_style.inverse = false,
                29 => self.current_style.strikethrough = false,
                // Standard foreground colors (30-37)
                30..=37 => {
                    self.current_style.fg = Some(colors::by_index((param - 30) as u8));
                }
                // Default foreground
                39 => self.current_style.fg = None,
                // Standard background colors (40-47)
                40..=47 => {
                    self.current_style.bg = Some(colors::by_index((param - 40) as u8));
                }
                // Default background
                49 => self.current_style.bg = None,
                // Bright foreground (90-97)
                90..=97 => {
                    self.current_style.fg = Some(colors::by_index((param - 90 + 8) as u8));
                }
                // Bright background (100-107)
                100..=107 => {
                    self.current_style.bg = Some(colors::by_index((param - 100 + 8) as u8));
                }
                // 256-color or RGB
                38 => {
                    if i + 1 < params.len() {
                        let mode = params[i + 1].first().copied().unwrap_or(0);
                        match mode {
                            5 if i + 2 < params.len() => {
                                // 256-color
                                let color_idx = params[i + 2].first().copied().unwrap_or(0) as u8;
                                self.current_style.fg = Some(colors::by_index(color_idx));
                                i += 2;
                            }
                            2 if i + 4 < params.len() => {
                                // RGB
                                let r = params[i + 2].first().copied().unwrap_or(0) as f32 / 255.0;
                                let g = params[i + 3].first().copied().unwrap_or(0) as f32 / 255.0;
                                let b = params[i + 4].first().copied().unwrap_or(0) as f32 / 255.0;
                                self.current_style.fg = Some(rgb_to_hsla(r, g, b));
                                i += 4;
                            }
                            _ => {}
                        }
                    }
                }
                48 => {
                    if i + 1 < params.len() {
                        let mode = params[i + 1].first().copied().unwrap_or(0);
                        match mode {
                            5 if i + 2 < params.len() => {
                                let color_idx = params[i + 2].first().copied().unwrap_or(0) as u8;
                                self.current_style.bg = Some(colors::by_index(color_idx));
                                i += 2;
                            }
                            2 if i + 4 < params.len() => {
                                let r = params[i + 2].first().copied().unwrap_or(0) as f32 / 255.0;
                                let g = params[i + 3].first().copied().unwrap_or(0) as f32 / 255.0;
                                let b = params[i + 4].first().copied().unwrap_or(0) as f32 / 255.0;
                                self.current_style.bg = Some(rgb_to_hsla(r, g, b));
                                i += 4;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl vte::Perform for AnsiParser {
    fn print(&mut self, c: char) {
        self.current_text.push(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            // Newline
            b'\n' => {
                self.current_text.push('\n');
            }
            // Carriage return
            b'\r' => {
                // Usually paired with \n, ignore on its own
            }
            // Tab
            b'\t' => {
                self.current_text.push_str("    ");
            }
            // Bell - ignore
            0x07 => {}
            // Backspace
            0x08 => {
                self.current_text.pop();
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // Flush current text before style change
        if !self.current_text.is_empty() {
            self.segments
                .push((std::mem::take(&mut self.current_text), self.current_style));
        }

        match action {
            // SGR - Select Graphic Rendition
            'm' => {
                let param_slices: Vec<&[u16]> = params.iter().collect();
                if param_slices.is_empty() {
                    // No params means reset
                    self.current_style = AnsiStyle::default();
                } else {
                    self.process_sgr(&param_slices);
                }
            }
            // Other CSI sequences (cursor movement, etc.) - ignored for now
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

/// Convert RGB to HSLA.
fn rgb_to_hsla(r: f32, g: f32, b: f32) -> Hsla {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsla::new(0.0, 0.0, l, 1.0);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    Hsla::new(h, s, l, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let mut parser = AnsiParser::new();
        let segments = parser.parse("Hello, world!");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "Hello, world!");
        assert_eq!(segments[0].1, AnsiStyle::default());
    }

    #[test]
    fn test_parse_colored_text() {
        let mut parser = AnsiParser::new();
        // Red text: \x1b[31m followed by reset
        let segments = parser.parse("\x1b[31mRed text\x1b[0m");

        // Only 1 segment because there's no text after the reset
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "Red text");
        assert!(segments[0].1.fg.is_some());
    }

    #[test]
    fn test_parse_bold() {
        let mut parser = AnsiParser::new();
        let segments = parser.parse("\x1b[1mBold\x1b[0m");

        // Only 1 segment because there's no text after the reset
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "Bold");
        assert!(segments[0].1.bold);
    }

    #[test]
    fn test_style_effective_colors() {
        let style = AnsiStyle::new().with_fg(colors::RED).with_bg(colors::BLUE);

        assert_eq!(style.effective_fg(), colors::RED);
        assert_eq!(style.effective_bg(), colors::BLUE);
    }

    #[test]
    fn test_style_inverse() {
        let mut style = AnsiStyle::new().with_fg(colors::RED).with_bg(colors::BLUE);
        style.inverse = true;

        assert_eq!(style.effective_fg(), colors::BLUE);
        assert_eq!(style.effective_bg(), colors::RED);
    }

    #[test]
    fn test_color_palette() {
        // Basic colors
        assert_eq!(colors::by_index(0), colors::BLACK);
        assert_eq!(colors::by_index(1), colors::RED);
        assert_eq!(colors::by_index(15), colors::BRIGHT_WHITE);
    }
}
