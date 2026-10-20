//! The desktop's dark theme (`openagents_ui::theme::Theme::dark`), as the
//! sRGB values its OKLCH tokens resolve to.

use gpui::{Font, FontFallbacks, FontStyle, FontWeight, Hsla, font, hsla, rgb};

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";
pub const CJK: &str = "Noto Sans CJK JP";

/// Body text size and line height in the transcript.
pub const BODY: f32 = 14.5;
pub const BODY_LINE: f32 = 23.0;
/// `typography::CODE_FONT_SIZE_DEFAULT`.
pub const CODE: f32 = 12.5;
pub const CODE_LINE: f32 = 19.0;

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Main panel, sampled `#060606`.
pub fn bg() -> Hsla {
    c(0x060606)
}
/// Shell and sidebar, sampled `#0d0d0d`.
pub fn surface() -> Hsla {
    c(0x0d0d0d)
}
pub fn surface_raised() -> Hsla {
    c(0x1f1f1f)
}
pub fn surface_card() -> Hsla {
    c(0x0e0e0e)
}
pub fn element_hover() -> Hsla {
    hsla(0.0, 0.0, 0.92, 0.07)
}
pub fn element_active() -> Hsla {
    hsla(0.0, 0.0, 0.92, 0.11)
}
pub fn border() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.08)
}
pub fn border_strong() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.14)
}
pub fn input_bg() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.03)
}
pub fn text() -> Hsla {
    c(0xe5e5e5)
}
pub fn text_muted() -> Hsla {
    c(0xa1a1a1)
}
pub fn text_faint() -> Hsla {
    c(0x737373)
}
pub fn solid() -> Hsla {
    c(0xe5e5e5)
}
pub fn on_solid() -> Hsla {
    c(0x0e0e0e)
}
/// Indigo-400, the default accent.
pub fn accent() -> Hsla {
    c(0x7c86ff)
}
pub fn accent_wash() -> Hsla {
    accent().opacity(0.12)
}
pub fn selection() -> Hsla {
    accent().opacity(0.35)
}
pub fn danger() -> Hsla {
    c(0xff6467)
}
pub fn warning() -> Hsla {
    c(0xfcbb00)
}
pub fn success() -> Hsla {
    c(0x00d294)
}
pub fn diff_add() -> Hsla {
    success()
}
pub fn diff_del() -> Hsla {
    danger()
}

/// Named fallbacks for Geist. Emoji isn't named: GPUI drops a named face
/// without a Latin `m`, so Noto Color Emoji is found by the text system's own
/// per-glyph fallback over the bundled faces instead.
pub fn fallbacks() -> FontFallbacks {
    FontFallbacks::from_fonts(vec![CJK.into()])
}

pub fn sans(weight: FontWeight, italic: bool) -> Font {
    Font {
        weight,
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        fallbacks: Some(fallbacks()),
        ..font(SANS)
    }
}

pub fn mono(weight: FontWeight) -> Font {
    Font {
        weight,
        fallbacks: Some(fallbacks()),
        ..font(MONO)
    }
}
