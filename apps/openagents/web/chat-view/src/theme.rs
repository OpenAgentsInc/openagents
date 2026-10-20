//! The desktop's dark theme for the web views: each color is read from the
//! desktop's own tokens (`openagents_chat_view::theme::Theme::dark`).

use std::sync::LazyLock;

use gpui::{Font, FontFallbacks, FontStyle, FontWeight, Hsla, font};
use openagents_chat_view::theme::Theme;

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";
pub const CJK: &str = "Noto Sans CJK JP";

/// Body text size and line height in the transcript.
pub const BODY: f32 = 14.5;
pub const BODY_LINE: f32 = 23.0;
/// `typography::CODE_FONT_SIZE_DEFAULT`.
pub const CODE: f32 = openagents_chat_view::typography::CODE_FONT_SIZE_DEFAULT;
pub const CODE_LINE: f32 = 19.0;

static DARK: LazyLock<Theme> = LazyLock::new(Theme::dark);

/// The desktop theme the web views draw with.
pub fn desktop() -> &'static Theme {
    &DARK
}

/// Main panel.
pub fn bg() -> Hsla {
    DARK.bg
}
/// Shell and sidebar.
pub fn surface() -> Hsla {
    DARK.surface
}
pub fn surface_raised() -> Hsla {
    DARK.surface_raised
}
pub fn surface_card() -> Hsla {
    DARK.surface_card
}
pub fn element_hover() -> Hsla {
    DARK.element_hover
}
pub fn element_active() -> Hsla {
    DARK.element_active
}
pub fn border() -> Hsla {
    DARK.border
}
pub fn border_strong() -> Hsla {
    DARK.border_strong
}
pub fn input_bg() -> Hsla {
    DARK.input_bg
}
pub fn text() -> Hsla {
    DARK.text
}
pub fn text_muted() -> Hsla {
    DARK.text_muted
}
pub fn text_faint() -> Hsla {
    DARK.text_faint
}
pub fn solid() -> Hsla {
    DARK.solid
}
pub fn on_solid() -> Hsla {
    DARK.on_solid
}
pub fn accent() -> Hsla {
    DARK.accent
}
/// The accent wash the desktop's notice cards and selected rows use
/// (`accent.opacity(0.12)`); `Theme::accent_wash` is the stronger identity
/// tint.
pub fn accent_wash() -> Hsla {
    DARK.accent.opacity(0.12)
}
pub fn selection() -> Hsla {
    DARK.selection
}
pub fn danger() -> Hsla {
    DARK.danger
}
pub fn warning() -> Hsla {
    DARK.warning
}
pub fn success() -> Hsla {
    DARK.success
}
pub fn diff_add() -> Hsla {
    DARK.diff_add
}
pub fn diff_del() -> Hsla {
    DARK.diff_del
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
