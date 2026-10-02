//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `xai-grok-pager-render/src/theme/color_support.rs`, `src/render/color.rs`,
//! the truecolor brand list of `src/terminal/mod.rs`, and the color mapping
//! of `src/syntax.rs`.
//!
//! Detects the terminal's color capabilities (truecolor / 256 / 16 / none).
//! [`quantize_color`] downgrades a [`ratatui::style::Color`] to the highest
//! level the terminal supports, and [`syntect_to_ratatui_fg`] turns a syntect
//! token into a foreground-only style at a level.
//!
//! One adaptation: grok-build routes syntect tokens through
//! [`polarity_safe_syntax_fg`] while its terminal-native lock holds, and that
//! lock caps the level at [`ColorLevel::Basic`]. OpenAgents Terminal has no
//! terminal-native theme, so the polarity-safe mapping is what
//! [`ColorLevel::Basic`] means here: on a 16-color terminal a night pastel
//! would otherwise collapse to White and vanish on a light profile.

use std::collections::HashMap;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};

/// Terminal color support level (ordered low to high).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColorLevel {
    /// No color support (monochrome).
    None,
    /// Basic 16-color ANSI (SGR 30–37 / 90–97).
    Basic,
    /// 256-color indexed palette (SGR 38;5;N).
    Ansi256,
    /// 24-bit truecolor RGB (SGR 38;2;R;G;B).
    TrueColor,
}

impl ColorLevel {
    pub fn has_color(self) -> bool {
        self >= Self::Basic
    }

    pub fn has_256(self) -> bool {
        self >= Self::Ansi256
    }

    pub fn has_truecolor(self) -> bool {
        self >= Self::TrueColor
    }
}

// ── Global singleton ─────────────────────────────────────────────────────

static COLOR_LEVEL: OnceLock<ColorLevel> = OnceLock::new();

/// `NO_COLOR` forces [`ColorLevel::None`]. Non-TTY without it defaults to
/// TrueColor (a TUI always runs in a terminal). Cached after the first call.
pub fn detect() -> ColorLevel {
    *COLOR_LEVEL.get_or_init(|| {
        let env: HashMap<String, String> = std::env::vars().collect();
        // Explicit opt-out via NO_COLOR takes priority.
        if env.contains_key("NO_COLOR") {
            return ColorLevel::None;
        }

        let level = match supports_color::on(supports_color::Stream::Stdout) {
            Some(level) => {
                if level.has_16m {
                    ColorLevel::TrueColor
                } else if level.has_256 {
                    ColorLevel::Ansi256
                } else if level.has_basic {
                    ColorLevel::Basic
                } else {
                    ColorLevel::None
                }
            }
            // `None` means stdout is not a TTY (tests, piped)
            None => ColorLevel::TrueColor,
        };

        // The `supports-color` crate relies on COLORTERM=truecolor, but tmux/SSH/mosh often strip that variable
        // When the crate reports only 256-color support, upgrade to TrueColor if we can identify the emulator and know it handles 24-bit RGB
        if level < ColorLevel::TrueColor && terminal_supports_truecolor(&env) {
            return ColorLevel::TrueColor;
        }

        level
    })
}

/// Return the cached color level (calls [`detect`] if not yet initialized).
pub fn get() -> ColorLevel {
    detect()
}

/// Override the color level (useful for tests or `--color` flags).
///
/// Returns `Err` if already set.
pub fn set(level: ColorLevel) -> Result<(), ColorLevel> {
    COLOR_LEVEL.set(level)
}

// ── Color quantization ──────────────────────────────────────────────────

/// Downgrade to what the terminal can show: Ansi256 nearest-index, Basic nearest ANSI16, None to `Reset`.
pub fn quantize_color(color: Color, level: ColorLevel) -> Color {
    match level {
        ColorLevel::TrueColor => color,
        ColorLevel::Ansi256 => match color {
            Color::Rgb(r, g, b) => Color::Indexed(nearest_indexed(r, g, b)),
            other => other,
        },
        ColorLevel::Basic => match color {
            Color::Rgb(r, g, b) => indexed_to_ansi16(nearest_indexed(r, g, b)),
            Color::Indexed(n) => indexed_to_ansi16(n),
            other => other,
        },
        ColorLevel::None => Color::Reset,
    }
}

/// Quantize a color using the globally-detected level.
pub fn quantize(color: Color) -> Color {
    quantize_color(color, get())
}

// ── Syntect tokens ──────────────────────────────────────────────────────

/// Convert syntect style to ratatui foreground-only style, quantized for
/// `level` (or polarity-safe on a 16-color terminal; see the module docs).
pub fn syntect_to_ratatui_fg(style: syntect::highlighting::Style, level: ColorLevel) -> Style {
    let fg = syntect_rgb_to_fg(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
        level,
    );
    let mut out = Style::default().fg(fg);
    use syntect::highlighting::FontStyle;
    if style.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

/// Map a syntect RGB triplet to a ratatui foreground color.
///
/// On a 16-color terminal, uses [`polarity_safe_syntax_fg`]; otherwise quantizes via the normal theme color pipeline.
pub fn syntect_rgb_to_fg(r: u8, g: u8, b: u8, level: ColorLevel) -> Color {
    if level == ColorLevel::Basic {
        polarity_safe_syntax_fg(r, g, b)
    } else {
        quantize_color(Color::Rgb(r, g, b), level)
    }
}

/// Low-chroma tokens use [`Color::Reset`] so host fg keeps contrast on both polarities.
/// Saturated hues map to base ANSI only — never White, Black, or Light*, which vanish on the opposite polarity.
pub fn polarity_safe_syntax_fg(r: u8, g: u8, b: u8) -> Color {
    let max = r.max(g).max(b) as i32;
    let min = r.min(g).min(b) as i32;
    let chroma = max - min;
    // Night default body (~#c8c8c8) and dim comments are near-gray.
    if chroma < 40 {
        return Color::Reset;
    }
    // Integer HSV hue in degrees [0, 360).
    let (ri, gi, bi) = (r as i32, g as i32, b as i32);
    let h = if max == ri {
        let mut h = (gi - bi) * 60 / chroma;
        if h < 0 {
            h += 360;
        }
        h
    } else if max == gi {
        (bi - ri) * 60 / chroma + 120
    } else {
        (ri - gi) * 60 / chroma + 240
    };
    // Magenta starts at 255° so Tokyo Night purple (#bb9af7, ~261°) lands Magenta rather than Blue; pure blues (~221°) stay Blue
    match h {
        0..30 | 330..=360 => Color::Red,
        30..90 => Color::Yellow,
        90..150 => Color::Green,
        150..210 => Color::Cyan,
        210..255 => Color::Blue,
        _ => Color::Magenta,
    }
}

// ── Terminal-based truecolor inference ──────────────────────────────────

/// Used as a fallback when `COLORTERM` is missing: inside tmux, SSH, or a
/// bare ConHost window. The brand is read from the same environment markers
/// grok-build's terminal detection reads.
fn terminal_supports_truecolor(env: &HashMap<String, String>) -> bool {
    truecolor_brand(env) || cfg!(target_os = "windows")
}

fn env_get<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(key).map(String::as_str).filter(|v| !v.is_empty())
}

/// Whether the environment names a terminal grok-build knows handles 24-bit
/// RGB: iTerm2, Ghostty, Kitty, WezTerm, Alacritty, Rio, Warp, VS Code,
/// Windows Terminal, foot.
pub fn truecolor_brand(env: &HashMap<String, String>) -> bool {
    if env_get(env, "CURSOR_TRACE_ID").is_some() {
        return false;
    }
    if let Some(askpass) = env_get(env, "VSCODE_GIT_ASKPASS_MAIN") {
        let lower = askpass.to_ascii_lowercase();
        // Cursor and Windsurf are not on the truecolor list; VS Code is.
        return !lower.contains("cursor") && !lower.contains("windsurf");
    }
    if let Some(term_program) = env_get(env, "TERM_PROGRAM") {
        let normalized: String = term_program
            .trim()
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '_' | '.'))
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match normalized.as_str() {
            "ghostty" | "iterm" | "iterm2" | "itermapp" | "warp" | "warpterminal" | "vscode"
            | "wezterm" | "kitty" | "alacritty" | "rio" | "windowsterminal" => return true,
            "appleterminal" | "terminator" | "zed" | "grokdesktop" | "otty" => return false,
            _ => {}
        }
    }
    if let Some(te) = env_get(env, "TERMINAL_EMULATOR") {
        let lower = te.to_ascii_lowercase();
        if lower.contains("jetbrains") || lower.contains("jediterm") {
            return false;
        }
    }
    if env_get(env, "WEZTERM_VERSION").is_some()
        || env_get(env, "ITERM_SESSION_ID").is_some()
        || env_get(env, "ITERM_PROFILE").is_some()
        || env_get(env, "LC_TERMINAL").is_some_and(|v| v.eq_ignore_ascii_case("iterm2"))
    {
        return true;
    }
    if env_get(env, "TERM_SESSION_ID").is_some() {
        return false;
    }
    let term = env_get(env, "TERM").unwrap_or("");
    if env_get(env, "KITTY_WINDOW_ID").is_some()
        || term.contains("kitty")
        || env_get(env, "ALACRITTY_SOCKET").is_some()
        || term == "alacritty"
        || term == "rio"
        || matches!(term, "foot" | "foot-extra" | "foot-direct")
    {
        return true;
    }
    if env_get(env, "TERMINATOR_UUID").is_some() || env_get(env, "VTE_VERSION").is_some() {
        return false;
    }
    env_get(env, "WT_SESSION").is_some()
}

// ── 256 → 16 mapping ────────────────────────────────────────────────────

/// Map a 256-color index to the nearest basic ANSI 16 color.
fn indexed_to_ansi16(n: u8) -> Color {
    match n {
        // First 16 indices already *are* the ANSI 16 colors.
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White, // actually "silver" in most terminals
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        // For 16–255, convert to RGB and find nearest ANSI 16 color.
        _ => {
            let (r, g, b) = indexed_to_rgb(n);
            rgb_to_ansi16(r, g, b)
        }
    }
}

/// Nearest xterm ANSI 16 by squared-Euclidean distance. Fallback only; 16-color terminals are rare.
fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Color {
    // Standard xterm ANSI 16 palette (same values used by indexed_to_rgb for 0–15).
    const PALETTE: [(u8, u8, u8, Color); 16] = [
        (0, 0, 0, Color::Black),
        (128, 0, 0, Color::Red),
        (0, 128, 0, Color::Green),
        (128, 128, 0, Color::Yellow),
        (0, 0, 128, Color::Blue),
        (128, 0, 128, Color::Magenta),
        (0, 128, 128, Color::Cyan),
        (192, 192, 192, Color::White),
        (128, 128, 128, Color::DarkGray),
        (255, 0, 0, Color::LightRed),
        (0, 255, 0, Color::LightGreen),
        (255, 255, 0, Color::LightYellow),
        (0, 0, 255, Color::LightBlue),
        (255, 0, 255, Color::LightMagenta),
        (0, 255, 255, Color::LightCyan),
        (255, 255, 255, Color::White), // index 15 = bright white
    ];

    let mut best = Color::White;
    let mut best_dist = u32::MAX;
    for &(pr, pg, pb, color) in &PALETTE {
        let dist = sq_dist(r, g, b, pr, pg, pb);
        if dist < best_dist {
            best_dist = dist;
            best = color;
        }
    }
    best
}

// ── 256-color palette ───────────────────────────────────────────────────

const CUBE_VALUES: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// 0–15 xterm ANSI, 16–231 color cube, 232–255 grayscale. A customized terminal palette will differ.
pub fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        // Standard colors (0-7): common xterm defaults
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        // Bright colors (8–15)
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        // 6×6×6 color cube (16–231)
        16..=231 => {
            let n = index - 16;
            // n is 0..=215, so each cube axis is 0..=5.
            let Some(&r) = CUBE_VALUES.get((n / 36) as usize) else {
                return (0, 0, 0);
            };
            let Some(&g) = CUBE_VALUES.get(((n % 36) / 6) as usize) else {
                return (0, 0, 0);
            };
            let Some(&b) = CUBE_VALUES.get((n % 6) as usize) else {
                return (0, 0, 0);
            };
            (r, g, b)
        }
        // Grayscale ramp (232–255): value = 8 + (index − 232) × 10
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            (v, v, v)
        }
    }
}

/// Nearest of the color cube and the grayscale ramp by squared Euclidean distance.
pub fn nearest_indexed(r: u8, g: u8, b: u8) -> u8 {
    // --- nearest in the 6×6×6 color cube (16–231) ---
    let ri = nearest_cube_channel(r);
    let gi = nearest_cube_channel(g);
    let bi = nearest_cube_channel(b);
    let cube_idx = 16 + 36 * ri as u16 + 6 * gi as u16 + bi as u16;
    let Some(&cube_r) = CUBE_VALUES.get(ri as usize) else {
        return 16;
    };
    let Some(&cube_g) = CUBE_VALUES.get(gi as usize) else {
        return 16;
    };
    let Some(&cube_b) = CUBE_VALUES.get(bi as usize) else {
        return 16;
    };
    let cube_dist = sq_dist(r, g, b, cube_r, cube_g, cube_b);

    // --- nearest in the grayscale ramp (232–255) ---
    // Ramp values: 8, 18, 28, …, 238  (24 entries)
    let lum = (r as u16 + g as u16 + b as u16) / 3;
    let gray_step = if lum <= 3 {
        0u8
    } else if lum >= 243 {
        23
    } else {
        ((lum as i16 - 8 + 5) / 10).clamp(0, 23) as u8
    };
    let gv = (8 + gray_step as u16 * 10) as u8;
    let gray_dist = sq_dist(r, g, b, gv, gv, gv);

    if gray_dist < cube_dist {
        232 + gray_step
    } else {
        cube_idx as u8
    }
}

/// Find the nearest index (0–5) into [`CUBE_VALUES`] for a single channel.
fn nearest_cube_channel(v: u8) -> u8 {
    let mut best = 0u8;
    let mut best_d = v.abs_diff(CUBE_VALUES[0]) as u16;
    for (i, &cube) in CUBE_VALUES.iter().enumerate().skip(1) {
        let d = v.abs_diff(cube) as u16;
        if d < best_d {
            best = i as u8;
            best_d = d;
        }
    }
    best
}

/// Squared Euclidean distance between two RGB colors.
fn sq_dist(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> u32 {
    let dr = r1 as i32 - r2 as i32;
    let dg = g1 as i32 - g2 as i32;
    let db = b1 as i32 - b2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

/// `None` only for `Reset`. Named colors use xterm 0-15 defaults; a customized terminal palette will differ.
pub fn resolve_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    let idx: u8 = match color {
        Color::Rgb(r, g, b) => return Some((r, g, b)),
        Color::Indexed(n) => return Some(indexed_to_rgb(n)),
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Reset => return None,
    };
    Some(indexed_to_rgb(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polarity_safe_grays_are_reset() {
        // Night default body / comments.
        assert_eq!(polarity_safe_syntax_fg(0xc8, 0xc8, 0xc8), Color::Reset);
        assert_eq!(polarity_safe_syntax_fg(0x6c, 0x6c, 0x6c), Color::Reset);
        assert_eq!(polarity_safe_syntax_fg(0xb2, 0xb2, 0xb2), Color::Reset);
        assert_eq!(polarity_safe_syntax_fg(0x44, 0x44, 0x44), Color::Reset);
    }

    #[test]
    fn polarity_safe_never_emits_white_or_black() {
        // Common night-theme pastels that naive ANSI16 maps to White.
        let samples = [
            (0xbb, 0x9a, 0xf7), // magenta
            (0x7d, 0xcf, 0xff), // cyan
            (0x7a, 0xa2, 0xf7), // blue
            (0xff, 0x9e, 0x64), // orange
            (0xf7, 0x76, 0x8e), // red
            (0xe0, 0xaf, 0x68), // yellow
            (0x9e, 0xce, 0x6a), // green
            (0xc8, 0xc8, 0xc8), // gray body
        ];
        for (r, g, b) in samples {
            let c = polarity_safe_syntax_fg(r, g, b);
            assert!(
                !matches!(
                    c,
                    Color::White
                        | Color::Black
                        | Color::Gray
                        | Color::DarkGray
                        | Color::LightRed
                        | Color::LightGreen
                        | Color::LightYellow
                        | Color::LightBlue
                        | Color::LightMagenta
                        | Color::LightCyan
                ),
                "polarity-unsafe color for #{r:02x}{g:02x}{b:02x}: {c:?}"
            );
        }
    }

    #[test]
    fn polarity_safe_chromatic_buckets() {
        assert_eq!(polarity_safe_syntax_fg(0xf7, 0x76, 0x8e), Color::Red);
        assert_eq!(polarity_safe_syntax_fg(0xe0, 0xaf, 0x68), Color::Yellow);
        assert_eq!(polarity_safe_syntax_fg(0x9e, 0xce, 0x6a), Color::Yellow); // Lime lands in the yellow bucket
        assert_eq!(polarity_safe_syntax_fg(0x7d, 0xcf, 0xff), Color::Cyan);
        assert_eq!(polarity_safe_syntax_fg(0x7a, 0xa2, 0xf7), Color::Blue);
        assert_eq!(polarity_safe_syntax_fg(0xbb, 0x9a, 0xf7), Color::Magenta);
    }

    #[test]
    fn basic_level_is_polarity_safe_and_others_quantize() {
        assert_eq!(
            syntect_rgb_to_fg(0xc8, 0xc8, 0xc8, ColorLevel::Basic),
            Color::Reset
        );
        assert_eq!(
            syntect_rgb_to_fg(0xbb, 0x9a, 0xf7, ColorLevel::Basic),
            Color::Magenta
        );
        assert_eq!(
            syntect_rgb_to_fg(0xbb, 0x9a, 0xf7, ColorLevel::TrueColor),
            Color::Rgb(0xbb, 0x9a, 0xf7)
        );
        assert_eq!(
            syntect_rgb_to_fg(0xbb, 0x9a, 0xf7, ColorLevel::Ansi256),
            Color::Indexed(nearest_indexed(0xbb, 0x9a, 0xf7))
        );
        assert_eq!(
            syntect_rgb_to_fg(0xbb, 0x9a, 0xf7, ColorLevel::None),
            Color::Reset
        );
    }

    #[test]
    fn nearest_indexed_matches_the_cube_and_ramp() {
        assert_eq!(nearest_indexed(0, 0, 0), 16);
        assert_eq!(nearest_indexed(255, 255, 255), 231);
        assert_eq!(nearest_indexed(128, 128, 128), 244);
        for index in 16..=255u8 {
            let (r, g, b) = indexed_to_rgb(index);
            assert_eq!(indexed_to_rgb(nearest_indexed(r, g, b)), (r, g, b));
        }
    }

    #[test]
    fn known_brands_are_truecolor() {
        let env = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        assert!(truecolor_brand(&env(&[("TERM_PROGRAM", "iTerm.app")])));
        assert!(truecolor_brand(&env(&[("TERM_PROGRAM", "ghostty")])));
        assert!(truecolor_brand(&env(&[("LC_TERMINAL", "iTerm2")])));
        assert!(truecolor_brand(&env(&[("TERM", "xterm-kitty")])));
        assert!(!truecolor_brand(&env(&[(
            "TERM_PROGRAM",
            "Apple_Terminal"
        )])));
        assert!(!truecolor_brand(&env(&[("TERM", "xterm-256color")])));
    }
}
