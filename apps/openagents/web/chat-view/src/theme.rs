//! The desktop's theme for the web views: each color is read from the
//! desktop's own tokens (`openagents_chat_view::theme::Theme`). It starts as
//! the dark theme; Settings → Appearance installs another with [`set`]
//! (#12289).

use std::cell::RefCell;
use std::rc::Rc;

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

thread_local! {
    static CURRENT: RefCell<Rc<Theme>> = RefCell::new(Rc::new(Theme::dark()));
}

fn with<T>(read: impl FnOnce(&Theme) -> T) -> T {
    CURRENT.with(|current| read(&current.borrow()))
}

/// The desktop theme the web views draw with.
pub fn desktop() -> Rc<Theme> {
    CURRENT.with(|current| current.borrow().clone())
}

/// Install the desktop theme as the GPUI global, for the desktop views the
/// web mounts directly, such as Rust Native cards
/// (`openagents_chat_view::rust_native_gpui`).
pub fn install(cx: &mut gpui::App) {
    set(Theme::dark(), cx);
}

/// Draw with `theme` from the next frame on.
pub fn set(theme: Theme, cx: &mut gpui::App) {
    openagents_chat_view::theme::set_current_appearance(theme.appearance);
    CURRENT.with(|current| *current.borrow_mut() = Rc::new(theme.clone()));
    cx.set_global(theme);
}

/// Main panel.
pub fn bg() -> Hsla {
    with(|t| t.bg)
}
/// Shell and sidebar.
pub fn surface() -> Hsla {
    with(|t| t.surface)
}
pub fn surface_raised() -> Hsla {
    with(|t| t.surface_raised)
}
pub fn surface_card() -> Hsla {
    with(|t| t.surface_card)
}
pub fn element_hover() -> Hsla {
    with(|t| t.element_hover)
}
pub fn element_active() -> Hsla {
    with(|t| t.element_active)
}
pub fn border() -> Hsla {
    with(|t| t.border)
}
pub fn border_strong() -> Hsla {
    with(|t| t.border_strong)
}
pub fn input_bg() -> Hsla {
    with(|t| t.input_bg)
}
pub fn text() -> Hsla {
    with(|t| t.text)
}
pub fn text_muted() -> Hsla {
    with(|t| t.text_muted)
}
pub fn text_faint() -> Hsla {
    with(|t| t.text_faint)
}
pub fn solid() -> Hsla {
    with(|t| t.solid)
}
pub fn on_solid() -> Hsla {
    with(|t| t.on_solid)
}
pub fn accent() -> Hsla {
    with(|t| t.accent)
}
/// The accent wash the desktop's notice cards and selected rows use
/// (`accent.opacity(0.12)`); `Theme::accent_wash` is the stronger identity
/// tint.
pub fn accent_wash() -> Hsla {
    with(|t| t.accent.opacity(0.12))
}
pub fn selection() -> Hsla {
    with(|t| t.selection)
}
pub fn danger() -> Hsla {
    with(|t| t.danger)
}
pub fn warning() -> Hsla {
    with(|t| t.warning)
}
pub fn success() -> Hsla {
    with(|t| t.success)
}
pub fn diff_add() -> Hsla {
    with(|t| t.diff_add)
}
pub fn diff_del() -> Hsla {
    with(|t| t.diff_del)
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
