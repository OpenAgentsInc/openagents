//! Main chat values reimplemented from Zeron's public dark theme and components,
//! and their Coder Light counterparts ([`Visual`]).
//! Reference: zeronsh/zeron 50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4 (MIT).
use rust_native::layout::{InlineCodeMetrics, MarkdownMetrics, Metrics, display::ColorRole};
use rust_native::style::Color;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

pub const CANVAS: Color = Color::rgb(6, 6, 6);
pub const SIDEBAR: Color = Color::rgb(13, 13, 13);
pub const COMPOSER: Color = Color::rgb(13, 13, 13);
pub const TEXT: Color = Color::rgb(229, 229, 229);
pub const MUTED: Color = Color::rgb(163, 163, 163);
pub const FAINT: Color = Color::rgb(115, 115, 115);
pub const SELECTED: Color = rgba(235, 235, 235, 28);
pub const BORDER: Color = rgba(255, 255, 255, 20);
pub const COMPOSER_BORDER: Color = rgba(189, 199, 209, 23);
pub const ACCENT: Color = Color::rgb(124, 134, 255);
const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
    Color {
        red,
        green,
        blue,
        alpha,
    }
}
pub const COLORS: [(ColorRole, Color); 9] = [
    (ColorRole::Primary, TEXT),
    (ColorRole::Secondary, MUTED),
    (ColorRole::Tertiary, FAINT),
    (ColorRole::Link, TEXT),
    (ColorRole::Bubble, rgba(235, 235, 235, 20)),
    (ColorRole::Surface, Color::rgb(14, 14, 14)),
    (ColorRole::Raised, Color::rgb(30, 30, 30)),
    (ColorRole::Border, BORDER),
    (ColorRole::InlineCode, rgba(124, 134, 255, 31)),
];
/// Zeron's dark syntax colors after its 72% HSL saturation treatment.
pub const SYNTAX: rust_native::syntax::Palette =
    rust_native::syntax::Palette::plain([229, 229, 229, 255])
        .with(rust_native::syntax::Kind::Keyword, [143, 150, 237, 255])
        .with(rust_native::syntax::Kind::Function, [143, 150, 237, 255])
        .with(
            rust_native::syntax::Kind::MarkupHeading,
            [143, 150, 237, 255],
        )
        .with(
            rust_native::syntax::Kind::MarkupStrong,
            [143, 150, 237, 255],
        )
        .with(
            rust_native::syntax::Kind::StringSpecial,
            [230, 121, 180, 255],
        )
        .with(rust_native::syntax::Kind::Escape, [230, 121, 180, 255])
        .with(
            rust_native::syntax::Kind::FunctionBuiltin,
            [230, 121, 180, 255],
        )
        .with(rust_native::syntax::Kind::Macro, [230, 121, 180, 255])
        .with(
            rust_native::syntax::Kind::VariableSpecial,
            [230, 121, 180, 255],
        )
        .with(rust_native::syntax::Kind::Tag, [230, 121, 180, 255])
        .with(rust_native::syntax::Kind::MarkupLink, [230, 121, 180, 255])
        .with(
            rust_native::syntax::Kind::MarkupEmphasis,
            [230, 121, 180, 255],
        )
        .with(rust_native::syntax::Kind::String, [30, 183, 135, 255])
        .with(rust_native::syntax::Kind::TypeBuiltin, [30, 183, 135, 255])
        .with(rust_native::syntax::Kind::Constant, [30, 183, 135, 255])
        .with(rust_native::syntax::Kind::MarkupRaw, [30, 183, 135, 255])
        .with(rust_native::syntax::Kind::Number, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Boolean, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Type, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Constructor, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Property, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Attribute, [219, 169, 36, 255])
        .with(rust_native::syntax::Kind::Label, [219, 169, 36, 255])
        .with(
            rust_native::syntax::Kind::MarkupReference,
            [219, 169, 36, 255],
        )
        .with(rust_native::syntax::Kind::Invalid, [233, 121, 124, 255])
        .with(rust_native::syntax::Kind::Comment, [128, 128, 128, 255]);
use oa_tokens::typography::conversation as type_scale;

const fn points(value: f32) -> u16 {
    value as u16
}

const fn heading(index: usize) -> [u16; 2] {
    let step = type_scale::HEADINGS[index];
    [points(step.size), points(step.line_height)]
}

/// The conversation's metrics: its text sizes are the web's chat page's,
/// from the shared type scale (`oa_tokens::typography::conversation`), on
/// the phone and the desktop alike (#11120).
pub const TRANSCRIPT: Metrics = Metrics {
    reading_width: 736,
    body_size: points(type_scale::BODY.size),
    body_line_height: points(type_scale::BODY.line_height),
    row_gap: 16,
    bubble_padding: 16,
    bubble_max_percent: 80,
    bubble_radius: 16,
    bubble_tail_radius: 16,
    markdown: Some(MarkdownMetrics {
        headings: [heading(0), heading(1), heading(2), heading(3)],
        code_size_half_points: points(type_scale::CODE.size * 2.0),
        code_line_height: points(type_scale::CODE.line_height),
        code_header_height: 28,
        code_label_size: points(type_scale::CODE_LABEL.size),
        code_padding_y: 10,
        copy_icon: true,
        strong_weight: Some(rust_native::layout::display::Weight::Semibold),
        inline_code: Some(InlineCodeMetrics {
            // `--markdown-code-font-size`: 0.875em of the body.
            size_percent: 88,
            inset_y: 2,
            radius_half_points: 9,
            color: [ACCENT.red, ACCENT.green, ACCENT.blue, ACCENT.alpha],
        }),
    }),
};

/// The chat surface's colors in one scheme: the seam a view reads its
/// colors from ([`current`]) instead of naming the dark constants above.
///
/// [`Visual::DARK`] is the dark chat look above, unchanged. [`Visual::LIGHT`]
/// is Coder Light, from the shared token table (`oa-tokens`), the same
/// values the web paints under `data-theme="light"` (#11028). The constants
/// above stay as the dark values for surfaces not yet moved to the seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub scheme: Scheme,
    pub canvas: Color,
    pub sidebar: Color,
    pub composer: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub selected: Color,
    pub border: Color,
    pub composer_border: Color,
    pub accent: Color,
    /// A floating control above the conversation (the scroll pill).
    pub raised: Color,
    /// A dialog's card.
    pub panel: Color,
    /// The base of translucent washes and hairlines drawn over a surface.
    pub ink: Color,
    /// A label on a `text`-filled button.
    pub on_text: Color,
    /// A `text`-filled button under the pointer.
    pub text_hover: Color,
    /// A warning note.
    pub warning: Color,
    /// Added and removed lines in a change, and their washes.
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_add_bg: Color,
    pub diff_remove_bg: Color,
    pub colors: [(ColorRole, Color); 9],
    pub syntax: rust_native::syntax::Palette,
    pub transcript: Metrics,
}

pub use oa_tokens::{Scheme, ThemeChoice};

const fn token(value: oa_tokens::Rgba8) -> Color {
    rgba(value.r, value.g, value.b, value.a)
}

const fn token_bytes(value: oa_tokens::Rgba8) -> [u8; 4] {
    [value.r, value.g, value.b, value.a]
}

const LIGHT_TOKENS: oa_tokens::Palette = oa_tokens::Palette::LIGHT;

/// Coder Light's syntax colors: the light intent text roles, each WCAG AA
/// on the light canvas.
const LIGHT_SYNTAX: rust_native::syntax::Palette = {
    use rust_native::syntax::Kind;
    let info = token_bytes(LIGHT_TOKENS.info);
    let discovery = token_bytes(LIGHT_TOKENS.discovery);
    let success = token_bytes(LIGHT_TOKENS.success);
    let warning = token_bytes(LIGHT_TOKENS.warning);
    rust_native::syntax::Palette::plain(token_bytes(LIGHT_TOKENS.content))
        .with(Kind::Keyword, info)
        .with(Kind::Function, info)
        .with(Kind::MarkupHeading, info)
        .with(Kind::MarkupStrong, info)
        .with(Kind::StringSpecial, discovery)
        .with(Kind::Escape, discovery)
        .with(Kind::FunctionBuiltin, discovery)
        .with(Kind::Macro, discovery)
        .with(Kind::VariableSpecial, discovery)
        .with(Kind::Tag, discovery)
        .with(Kind::MarkupLink, discovery)
        .with(Kind::MarkupEmphasis, discovery)
        .with(Kind::String, success)
        .with(Kind::TypeBuiltin, success)
        .with(Kind::Constant, success)
        .with(Kind::MarkupRaw, success)
        .with(Kind::Number, warning)
        .with(Kind::Boolean, warning)
        .with(Kind::Type, warning)
        .with(Kind::Constructor, warning)
        .with(Kind::Property, warning)
        .with(Kind::Attribute, warning)
        .with(Kind::Label, warning)
        .with(Kind::MarkupReference, warning)
        .with(Kind::Invalid, token_bytes(LIGHT_TOKENS.danger))
        .with(Kind::Comment, token_bytes(LIGHT_TOKENS.content_secondary))
};

const NOIR_TOKENS: oa_tokens::Palette = oa_tokens::Palette::NOIR;

/// Coder Noir's syntax colors: Noir's intent text roles.
const NOIR_SYNTAX: rust_native::syntax::Palette = {
    use rust_native::syntax::Kind;
    let info = token_bytes(NOIR_TOKENS.info);
    let discovery = token_bytes(NOIR_TOKENS.discovery);
    let success = token_bytes(NOIR_TOKENS.success);
    let warning = token_bytes(NOIR_TOKENS.warning);
    rust_native::syntax::Palette::plain(token_bytes(NOIR_TOKENS.content))
        .with(Kind::Keyword, info)
        .with(Kind::Function, info)
        .with(Kind::MarkupHeading, info)
        .with(Kind::MarkupStrong, info)
        .with(Kind::StringSpecial, discovery)
        .with(Kind::Escape, discovery)
        .with(Kind::FunctionBuiltin, discovery)
        .with(Kind::Macro, discovery)
        .with(Kind::VariableSpecial, discovery)
        .with(Kind::Tag, discovery)
        .with(Kind::MarkupLink, discovery)
        .with(Kind::MarkupEmphasis, discovery)
        .with(Kind::String, success)
        .with(Kind::TypeBuiltin, success)
        .with(Kind::Constant, success)
        .with(Kind::MarkupRaw, success)
        .with(Kind::Number, warning)
        .with(Kind::Boolean, warning)
        .with(Kind::Type, warning)
        .with(Kind::Constructor, warning)
        .with(Kind::Property, warning)
        .with(Kind::Attribute, warning)
        .with(Kind::Label, warning)
        .with(Kind::MarkupReference, warning)
        .with(Kind::Invalid, token_bytes(NOIR_TOKENS.danger))
        .with(Kind::Comment, token_bytes(NOIR_TOKENS.content_secondary))
};

/// The web's dark ghost wash (`--color-background-primary-ghost-hover`,
/// `--alpha-12` of the text), the fill of a hovered or current row.
const NOIR_WASH: Color = token(NOIR_TOKENS.content.with_alpha(31));

/// The dark transcript metrics with the inline-code ink in `color`.
const fn transcript_inked(color: Color) -> Metrics {
    let mut metrics = TRANSCRIPT;
    if let Some(mut markdown) = metrics.markdown {
        if let Some(mut code) = markdown.inline_code {
            code.color = [color.red, color.green, color.blue, color.alpha];
            markdown.inline_code = Some(code);
        }
        metrics.markdown = Some(markdown);
    }
    metrics
}

impl Visual {
    /// The dark chat look.
    pub const DARK: Visual = Visual {
        scheme: Scheme::Dark,
        canvas: CANVAS,
        sidebar: SIDEBAR,
        composer: COMPOSER,
        text: TEXT,
        muted: MUTED,
        faint: FAINT,
        selected: SELECTED,
        border: BORDER,
        composer_border: COMPOSER_BORDER,
        accent: ACCENT,
        raised: Color::rgb(32, 32, 32),
        panel: Color::rgb(16, 16, 16),
        ink: Color::rgb(255, 255, 255),
        on_text: Color::rgb(14, 14, 14),
        text_hover: Color::rgb(206, 206, 206),
        warning: Color::rgb(229, 192, 123),
        diff_add: Color::rgb(163, 190, 140),
        diff_remove: Color::rgb(191, 120, 120),
        diff_add_bg: Color::rgb(28, 48, 34),
        diff_remove_bg: Color::rgb(58, 32, 36),
        colors: COLORS,
        syntax: SYNTAX,
        transcript: TRANSCRIPT,
    };

    /// Coder Light.
    pub const LIGHT: Visual = Visual {
        scheme: Scheme::Light,
        canvas: token(LIGHT_TOKENS.canvas),
        sidebar: token(LIGHT_TOKENS.surface_subtle),
        composer: token(LIGHT_TOKENS.surface_raised),
        text: token(LIGHT_TOKENS.content),
        muted: token(LIGHT_TOKENS.content_secondary),
        faint: token(LIGHT_TOKENS.content_tertiary),
        selected: token(LIGHT_TOKENS.surface),
        border: token(LIGHT_TOKENS.stroke_subtle),
        composer_border: token(LIGHT_TOKENS.stroke),
        accent: token(LIGHT_TOKENS.accent),
        raised: token(LIGHT_TOKENS.surface_raised),
        panel: token(LIGHT_TOKENS.surface_raised),
        ink: token(LIGHT_TOKENS.content),
        on_text: token(LIGHT_TOKENS.accent_on_solid),
        text_hover: token(LIGHT_TOKENS.content_secondary),
        warning: token(LIGHT_TOKENS.warning),
        diff_add: token(LIGHT_TOKENS.success),
        diff_remove: token(LIGHT_TOKENS.danger),
        diff_add_bg: token(LIGHT_TOKENS.success_container),
        diff_remove_bg: token(LIGHT_TOKENS.danger_container),
        colors: [
            (ColorRole::Primary, token(LIGHT_TOKENS.content)),
            (ColorRole::Secondary, token(LIGHT_TOKENS.content_secondary)),
            (ColorRole::Tertiary, token(LIGHT_TOKENS.content_tertiary)),
            (ColorRole::Link, token(LIGHT_TOKENS.content)),
            (ColorRole::Bubble, token(LIGHT_TOKENS.surface)),
            (ColorRole::Surface, token(LIGHT_TOKENS.surface_subtle)),
            (ColorRole::Raised, token(LIGHT_TOKENS.surface)),
            (ColorRole::Border, token(LIGHT_TOKENS.stroke_subtle)),
            (
                ColorRole::InlineCode,
                token(LIGHT_TOKENS.accent.with_alpha(31)),
            ),
        ],
        syntax: LIGHT_SYNTAX,
        transcript: transcript_inked(token(LIGHT_TOKENS.accent)),
    };

    /// Coder Noir: the web's dark values from the shared token table, the
    /// desktop's dark look (#11120, [`use_noir`]).
    pub const NOIR: Visual = Visual {
        scheme: Scheme::Dark,
        canvas: token(NOIR_TOKENS.canvas),
        sidebar: token(NOIR_TOKENS.surface_subtle),
        composer: token(NOIR_TOKENS.surface_raised),
        text: token(NOIR_TOKENS.content),
        muted: token(NOIR_TOKENS.content_secondary),
        faint: token(NOIR_TOKENS.content_tertiary),
        selected: NOIR_WASH,
        border: token(NOIR_TOKENS.stroke_subtle),
        composer_border: token(NOIR_TOKENS.stroke),
        accent: token(NOIR_TOKENS.accent),
        raised: token(NOIR_TOKENS.surface_raised),
        panel: token(NOIR_TOKENS.surface_raised),
        ink: token(NOIR_TOKENS.content),
        on_text: token(NOIR_TOKENS.accent_on_solid),
        text_hover: token(NOIR_TOKENS.content_secondary),
        warning: token(NOIR_TOKENS.warning),
        diff_add: token(NOIR_TOKENS.success),
        diff_remove: token(NOIR_TOKENS.danger),
        diff_add_bg: token(NOIR_TOKENS.success_container),
        diff_remove_bg: token(NOIR_TOKENS.danger_container),
        colors: [
            (ColorRole::Primary, token(NOIR_TOKENS.content)),
            (ColorRole::Secondary, token(NOIR_TOKENS.content_secondary)),
            (ColorRole::Tertiary, token(NOIR_TOKENS.content_tertiary)),
            (ColorRole::Link, token(NOIR_TOKENS.content)),
            (ColorRole::Bubble, NOIR_WASH),
            (ColorRole::Surface, token(NOIR_TOKENS.surface)),
            (ColorRole::Raised, token(NOIR_TOKENS.surface_raised)),
            (ColorRole::Border, token(NOIR_TOKENS.stroke_subtle)),
            (
                ColorRole::InlineCode,
                token(NOIR_TOKENS.content.with_alpha(31)),
            ),
        ],
        syntax: NOIR_SYNTAX,
        transcript: transcript_inked(token(NOIR_TOKENS.content)),
    };

    /// The look for `scheme`.
    #[must_use]
    pub const fn of(scheme: Scheme) -> &'static Visual {
        match scheme {
            Scheme::Light => &Visual::LIGHT,
            Scheme::Dark => &Visual::DARK,
        }
    }
}

/// The scheme the app paints with, set once at the app's theme seam
/// ([`set_scheme`]): dark until an app says otherwise, so a surface that
/// never sets it (the phones, today) keeps the dark look.
static LIGHT_ACTIVE: AtomicBool = AtomicBool::new(false);

std::thread_local! {
    /// A scheme kept to this thread by [`scoped`]; while it is set,
    /// [`set_scheme`] and [`scheme`] use it, not the process's.
    static SCOPED: Cell<Option<Scheme>> = const { Cell::new(None) };
}

/// Sets the scheme every view reading [`current`] paints with. The app
/// resolves it from the person's [`ThemeChoice`] and the system appearance,
/// then rebuilds its views. Inside [`scoped`] it sets only this thread's.
pub fn set_scheme(scheme: Scheme) {
    let inside = SCOPED.with(|scoped| {
        let inside = scoped.get().is_some();
        if inside {
            scoped.set(Some(scheme));
        }
        inside
    });
    if !inside {
        LIGHT_ACTIVE.store(scheme == Scheme::Light, Ordering::Relaxed);
    }
}

/// The scheme set by [`set_scheme`].
#[must_use]
pub fn scheme() -> Scheme {
    if let Some(scheme) = SCOPED.with(Cell::get) {
        return scheme;
    }
    if LIGHT_ACTIVE.load(Ordering::Relaxed) {
        Scheme::Light
    } else {
        Scheme::Dark
    }
}

/// Runs `f` with the scheme kept to this thread: it starts at the
/// process's scheme, [`set_scheme`] inside changes only this thread's, and
/// it is dropped when `f` returns or panics. Tests switch schemes in here
/// so they never change the scheme other tests paint with.
pub fn scoped<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Scheme>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0;
            SCOPED.with(|scoped| scoped.set(previous));
        }
    }
    let start = scheme();
    let _restore = Restore(SCOPED.with(|scoped| scoped.replace(Some(start))));
    f()
}

/// Whether the dark look is [`Visual::NOIR`] ([`use_noir`]).
static NOIR_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Paints the dark scheme in [`Visual::NOIR`], the web's dark tokens,
/// instead of [`Visual::DARK`]. The desktop calls it once at start
/// (#11120); the phones keep their dark look.
pub fn use_noir() {
    NOIR_ACTIVE.store(true, Ordering::Relaxed);
}

/// The look for the scheme the app paints with.
#[must_use]
pub fn current() -> &'static Visual {
    match scheme() {
        Scheme::Dark if NOIR_ACTIVE.load(Ordering::Relaxed) => &Visual::NOIR,
        scheme => Visual::of(scheme),
    }
}

/// `dark` in the dark look and `light` in Coder Light, for a surface's own
/// color that has no role in [`Visual`]; the dark value stays exactly what
/// the surface painted before it followed the theme.
#[must_use]
pub fn pick(dark: Color, light: Color) -> Color {
    match scheme() {
        Scheme::Dark => dark,
        Scheme::Light => light,
    }
}

/// The few colors the shared Rust Native views (the phones' chat, its
/// cards and panels, and the change pane) name outright, in one scheme.
///
/// [`Inks::DARK`] keeps the values those views always painted on the
/// phones' black, so the dark look is unchanged; [`Inks::LIGHT`] is Coder
/// Light's roles from the token table (#11028). Views read [`inks`] when
/// they build, so a tree built after [`set_scheme`] paints in the new look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inks {
    /// Primary text and buttons.
    pub text: Color,
    /// Status lines, tool rows, and other receded text.
    pub quiet: Color,
    /// A card or panel's fill in the transcript.
    pub card: Color,
    /// A warning note.
    pub warning: Color,
    /// Added and removed lines in a change.
    pub added: Color,
    pub removed: Color,
    /// A landing badge: landed, waiting on someone, or failed.
    pub done: Color,
    pub open: Color,
    pub attention: Color,
    /// An approval's risk chip fill: low, medium, high.
    pub risk_low: Color,
    pub risk_medium: Color,
    pub risk_high: Color,
}

impl Inks {
    /// The phones' established dark values.
    pub const DARK: Inks = Inks {
        text: Color::rgb(255, 255, 255),
        quiet: Color::rgb(153, 153, 153),
        card: Color::rgb(26, 29, 34),
        warning: Color::rgb(229, 192, 123),
        added: Color::rgb(163, 190, 140),
        removed: Color::rgb(191, 120, 120),
        done: Color::rgb(87, 196, 128),
        open: Color::rgb(232, 176, 72),
        attention: Color::rgb(232, 98, 92),
        risk_low: Color::rgb(46, 92, 64),
        risk_medium: Color::rgb(122, 92, 28),
        risk_high: Color::rgb(128, 40, 40),
    };

    /// Coder Light. Text roles are the intent text tokens (WCAG AA on the
    /// light canvas); fills are the soft intent backgrounds, so the
    /// default (dark) label stays readable on them.
    pub const LIGHT: Inks = Inks {
        text: token(LIGHT_TOKENS.content),
        quiet: token(LIGHT_TOKENS.content_secondary),
        card: token(LIGHT_TOKENS.surface),
        warning: token(LIGHT_TOKENS.warning),
        added: token(LIGHT_TOKENS.success),
        removed: token(LIGHT_TOKENS.danger),
        done: token(LIGHT_TOKENS.success),
        open: token(LIGHT_TOKENS.warning),
        attention: token(LIGHT_TOKENS.danger),
        risk_low: token(LIGHT_TOKENS.success_container),
        risk_medium: token(LIGHT_TOKENS.warning_container),
        risk_high: token(LIGHT_TOKENS.danger_container),
    };

    /// Coder Noir, the desktop's dark look ([`use_noir`]): the web's dark
    /// roles from the token table.
    pub const NOIR: Inks = Inks {
        text: token(NOIR_TOKENS.content),
        quiet: token(NOIR_TOKENS.content_secondary),
        card: token(NOIR_TOKENS.surface_raised),
        warning: token(NOIR_TOKENS.warning),
        added: token(NOIR_TOKENS.success),
        removed: token(NOIR_TOKENS.danger),
        done: token(NOIR_TOKENS.success),
        open: token(NOIR_TOKENS.warning),
        attention: token(NOIR_TOKENS.danger),
        risk_low: token(NOIR_TOKENS.success_container),
        risk_medium: token(NOIR_TOKENS.warning_container),
        risk_high: token(NOIR_TOKENS.danger_container),
    };

    /// The inks for `scheme`.
    #[must_use]
    pub const fn of(scheme: Scheme) -> &'static Inks {
        match scheme {
            Scheme::Light => &Inks::LIGHT,
            Scheme::Dark => &Inks::DARK,
        }
    }
}

/// The inks for the scheme the app paints with.
#[must_use]
pub fn inks() -> &'static Inks {
    match scheme() {
        Scheme::Dark if NOIR_ACTIVE.load(Ordering::Relaxed) => &Inks::NOIR,
        scheme => Inks::of(scheme),
    }
}

/// The shared token table's roles for the scheme the app paints with:
/// Coder Light, or Coder Noir in the dark. A surface with no role in
/// [`Visual`] paints from here ([`role`]) instead of its own colors.
#[must_use]
pub fn palette() -> &'static oa_tokens::Palette {
    oa_tokens::Palette::of(scheme())
}

/// A token table color as a paint color.
#[must_use]
pub const fn role(value: oa_tokens::Rgba8) -> Color {
    token(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::layout::{
        TranscriptLayout, Update,
        display::{FontFamily, Ink},
        testing::FixedMeasurer,
    };
    use rust_native::style::Style;
    use rust_native::{Element, MessageRole, Node};

    /// The shared views' inks: the dark set is what the phones always
    /// painted, the light set Coder Light's roles.
    #[test]
    fn the_noir_look_is_the_webs_dark_tokens() {
        let resolver = oa_tokens::Resolver::new();
        let dark = |name: &str| {
            resolver
                .color(name, Scheme::Dark)
                .map(oa_tokens::Rgba::to_8bit)
                .unwrap_or_else(|| panic!("{name}"))
        };
        let rgb = |color: Color| (color.red, color.green, color.blue);
        for (color, name) in [
            (Visual::NOIR.canvas, "--color-surface"),
            (Visual::NOIR.sidebar, "--color-surface-secondary"),
            (Visual::NOIR.text, "--color-text"),
            (Visual::NOIR.muted, "--color-text-secondary"),
            (Visual::NOIR.faint, "--color-text-tertiary"),
        ] {
            let token = dark(name);
            assert_eq!(rgb(color), (token.r, token.g, token.b), "{name}");
        }
        assert_eq!(Visual::NOIR.scheme, Scheme::Dark);
    }

    #[test]
    fn the_inks_follow_the_scheme() {
        assert_eq!(Inks::of(Scheme::Dark), &Inks::DARK);
        assert_eq!(Inks::of(Scheme::Light), &Inks::LIGHT);
        assert_eq!(Inks::DARK.text, Color::rgb(255, 255, 255));
        assert_eq!(Inks::DARK.quiet, Color::rgb(153, 153, 153));
        let tokens = oa_tokens::Palette::LIGHT;
        assert_eq!(Inks::LIGHT.text, token(tokens.content));
        assert_eq!(Inks::LIGHT.quiet, token(tokens.content_secondary));
        assert_eq!(Inks::LIGHT.card, token(tokens.surface));
        assert_eq!(Inks::LIGHT.risk_high, token(tokens.danger_container));
    }

    /// The dark look is the chat's established dark values; the light look
    /// is Coder Light from the shared token table, at the same geometry.
    #[test]
    fn the_light_look_is_coder_light_and_the_dark_look_is_unchanged() {
        assert_eq!(Visual::DARK.canvas, CANVAS);
        assert_eq!(Visual::DARK.text, TEXT);
        assert_eq!(Visual::DARK.colors, COLORS);
        assert_eq!(Visual::DARK.syntax, SYNTAX);
        assert_eq!(Visual::DARK.transcript, TRANSCRIPT);
        assert_eq!(Visual::of(Scheme::Dark), &Visual::DARK);
        assert_eq!(Visual::of(Scheme::Light), &Visual::LIGHT);

        let tokens = oa_tokens::Palette::LIGHT;
        assert_eq!(Visual::LIGHT.canvas, token(tokens.canvas));
        assert_eq!(Visual::LIGHT.sidebar, token(tokens.surface_subtle));
        assert_eq!(Visual::LIGHT.text, token(tokens.content));
        assert_eq!(Visual::LIGHT.muted, token(tokens.content_secondary));
        assert_eq!(Visual::LIGHT.border, token(tokens.stroke_subtle));
        assert_eq!(Visual::LIGHT.accent, token(tokens.accent));
        let light = Visual::LIGHT.transcript;
        assert_eq!(
            light.markdown.and_then(|m| m.inline_code).map(|c| c.color),
            Some(token_bytes(tokens.accent))
        );
        assert_eq!(
            Metrics {
                markdown: TRANSCRIPT.markdown,
                ..light
            },
            TRANSCRIPT,
            "only the inline-code ink differs"
        );
        // Light text on the light field, dark on dark.
        let luma = |c: Color| {
            (u32::from(c.red) * 2126 + u32::from(c.green) * 7152 + u32::from(c.blue) * 722) / 10_000
        };
        assert!(luma(Visual::LIGHT.canvas) > 200 && luma(Visual::LIGHT.text) < 32);
        assert!(luma(Visual::DARK.canvas) < 32 && luma(Visual::DARK.text) > 200);
    }

    /// A scoped scheme changes only this thread's look, and the process's
    /// returns when the scope ends.
    #[test]
    fn a_scoped_scheme_stays_on_its_thread_and_ends_with_the_scope() {
        let outside = scheme();
        scoped(|| {
            set_scheme(Scheme::Light);
            assert_eq!(current(), &Visual::LIGHT);
            assert_eq!(map::current(), &map::Kinds::LIGHT);
            let other = std::thread::spawn(scheme).join().unwrap();
            assert_eq!(other, outside, "another thread keeps the process's");
            set_scheme(Scheme::Dark);
            assert_eq!(current(), &Visual::DARK);
            set_scheme(Scheme::Light);
        });
        assert_eq!(scheme(), outside);
    }

    #[test]
    fn reference_inline_code_keeps_text_ranges_and_scales_its_inset_wash() {
        for scale in [1.0, 2.0] {
            let mut layout = TranscriptLayout::new();
            layout.set_metrics(TRANSCRIPT).unwrap();
            layout
                .update(
                    Update {
                        width: 768.0,
                        scale,
                        rows: vec![Node {
                            key: "inline".into(),
                            style: Style::default(),
                            element: Element::Markdown {
                                blocks: rust_native::markdown::parse(
                                    "Use **strong** `café` and [the guide](https://example.com).",
                                ),
                            },
                        }],
                        order: Some(vec!["inline".into()]),
                        ..Update::default()
                    },
                    &mut FixedMeasurer::default(),
                )
                .unwrap();
            let frame = layout.frame();
            let row = frame.display(0).unwrap();
            let run = row
                .runs
                .iter()
                .find(|run| {
                    let text = &row.texts[run.text as usize];
                    &text[run.start8 as usize..(run.start8 + run.len8) as usize] == "café"
                })
                .unwrap();
            let style = row.styles[run.style as usize];
            assert!(style.font.mono);
            assert!((style.font.size - 16.0 * scale * 0.88).abs() < 0.01);
            assert_eq!(style.ink, Ink::Rgba([124, 134, 255, 255]));
            let wash = row
                .rects
                .iter()
                .find(|rect| rect.fill == Some(Ink::Role(ColorRole::InlineCode)))
                .unwrap();
            assert_eq!(wash.h, 20.0 * scale);
            assert_eq!(wash.radii, [4.5 * scale; 4]);
            assert_eq!(row.links[0].destination, "https://example.com");
            let strong = row
                .runs
                .iter()
                .find(|run| {
                    let text = &row.texts[run.text as usize];
                    &text[run.start8 as usize..(run.start8 + run.len8) as usize] == "strong"
                })
                .unwrap();
            assert_eq!(
                row.styles[strong.style as usize].font.weight,
                rust_native::layout::display::Weight::Semibold
            );
        }
    }

    #[test]
    fn reference_syntax_colors_preserve_utf8_source_ranges() {
        let source = "// café\nlet answer = 42;\nprintln!(\"hello\");\n";
        let spans = rust_native::syntax::Highlighter::with_palette(SYNTAX).spans("rust", source);
        for (token, color) in [
            ("// café", [128, 128, 128, 255]),
            ("let", [143, 150, 237, 255]),
            ("42", [219, 169, 36, 255]),
            ("println", [230, 121, 180, 255]),
            ("hello", [30, 183, 135, 255]),
        ] {
            let start = source.find(token).unwrap();
            assert!(
                spans.iter().any(|span| {
                    span.start <= start
                        && span.end >= start + token.len()
                        && span.foreground == color
                }),
                "missing reference color for {token}"
            );
        }
        assert!(
            spans
                .iter()
                .all(|span| source.get(span.start..span.end).is_some())
        );
    }

    #[test]
    fn reference_heading_and_code_metrics_preserve_copy_bytes() {
        use rust_native::layout::display::{Weight, WidgetKind};
        let node = Node {
            key: "markdown".into(),
            style: Style::default(),
            element: Element::Markdown {
                blocks: rust_native::markdown::parse(
                    "# Heading one\n\n## Heading two\n\n### Heading three\n\n#### Heading four\n\n```rust\nlet answer = 42;\n```",
                ),
            },
        };
        let mut layout = TranscriptLayout::new();
        layout.set_font_family(FontFamily::PaperMono);
        layout.set_metrics(TRANSCRIPT).unwrap();
        layout
            .update(
                Update {
                    width: 768.0,
                    scale: 1.0,
                    rows: vec![node],
                    order: Some(vec!["markdown".into()]),
                    ..Update::default()
                },
                &mut FixedMeasurer::default(),
            )
            .unwrap();
        let frame = layout.frame();
        let row = frame.display(0).unwrap();
        for size in [24.0, 20.0, 18.0, 16.0] {
            assert!(
                row.styles
                    .iter()
                    .any(|s| s.font.size == size && s.font.weight == Weight::Semibold)
            );
        }
        assert!(
            row.styles
                .iter()
                .any(|s| s.font.size == 14.0 && s.font.mono)
        );
        let code = row
            .rects
            .iter()
            .find(|r| r.fill == Some(Ink::Role(ColorRole::Surface)))
            .unwrap();
        assert_eq!(code.h, 70.0);
        let copy=row.widgets.iter().find(|w| matches!(&w.kind, WidgetKind::Copy {text,icon:true} if text == "let answer = 42;\n")).unwrap();
        assert_eq!((copy.w, copy.h), (24.0, 22.0));
        assert_eq!(copy.y - code.y, 3.0);
    }

    #[test]
    fn reference_body_and_bubble_geometry_use_one_shared_reading_band() {
        let node = Node {
            key: "message".into(),
            style: Style::default(),
            element: Element::Message {
                role: MessageRole::User,
                note: None,
                children: vec![Node {
                    key: "body".into(),
                    style: Style::default(),
                    element: Element::Markdown {
                        blocks: rust_native::markdown::parse(
                            &"A long prompt with spaces. ".repeat(80),
                        ),
                    },
                }],
            },
        };
        let mut layout = TranscriptLayout::new();
        layout.set_font_family(FontFamily::PaperMono);
        layout.set_metrics(TRANSCRIPT).unwrap();
        let update = || Update {
            width: 768.0,
            scale: 1.0,
            rows: vec![node.clone()],
            order: Some(vec!["message".into()]),
            ..Update::default()
        };
        let mut measurer = FixedMeasurer::default();
        layout.update(update(), &mut measurer).unwrap();
        let frame = layout.frame();
        let row = frame.display(0).unwrap();
        let bubble = row
            .rects
            .iter()
            .find(|rect| rect.fill == Some(Ink::Role(ColorRole::Bubble)))
            .unwrap();
        assert_eq!(bubble.radii, [16.0; 4]);
        assert!((bubble.x + bubble.w - 752.0).abs() < 0.01);
        assert!((bubble.w - 736.0 * 0.8).abs() < 0.01);
        assert!(((bubble.h - 20.0) % 24.0).abs() < 0.01);
        assert!(
            row.styles
                .iter()
                .all(|style| style.font.size == 16.0 && style.font.family == FontFamily::PaperMono)
        );
        assert_eq!(layout.update(update(), &mut measurer).unwrap().relaid, 0);
        let mut invalid = TRANSCRIPT;
        invalid.bubble_max_percent = 101;
        assert!(layout.set_metrics(invalid).is_err());
    }
}

/// The route map's kind colors (#10085): one hue per node kind on the dark
/// canvas, named by the map's legend. Health is the ring, never the fill.
pub mod map {
    use rust_native::style::Color;

    pub const FRONT: Color = super::TEXT;
    pub const FAMILY: Color = Color::rgb(128, 128, 136);
    pub const ROUTE: Color = super::ACCENT;
    pub const ANSWER: Color = Color::rgb(77, 196, 180);
    pub const KNOWLEDGE: Color = Color::rgb(132, 196, 98);
    pub const MODEL: Color = Color::rgb(150, 164, 186);
    pub const CODER: Color = Color::rgb(186, 140, 255);
    pub const ENGINE: Color = Color::rgb(230, 121, 180);
    pub const PLUGIN: Color = Color::rgb(242, 162, 72);
    pub const SCREEN: Color = Color::rgb(224, 204, 96);
    /// A gap's marker.
    pub const GAP: Color = Color::rgb(244, 86, 86);
    /// The ring of a node measured and weak.
    pub const WEAK: Color = Color::rgb(255, 120, 120);
    /// An edge.
    pub const EDGE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 38,
    };

    /// The map's colors in one scheme: the dark values above, or their
    /// Coder Light counterparts, darker and more saturated so each reads
    /// on the light canvas.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Kinds {
        pub front: Color,
        pub family: Color,
        pub route: Color,
        pub answer: Color,
        pub knowledge: Color,
        pub model: Color,
        pub coder: Color,
        pub engine: Color,
        pub plugin: Color,
        pub screen: Color,
        pub gap: Color,
        pub weak: Color,
        pub edge: Color,
    }

    impl Kinds {
        pub const DARK: Kinds = Kinds {
            front: FRONT,
            family: FAMILY,
            route: ROUTE,
            answer: ANSWER,
            knowledge: KNOWLEDGE,
            model: MODEL,
            coder: CODER,
            engine: ENGINE,
            plugin: PLUGIN,
            screen: SCREEN,
            gap: GAP,
            weak: WEAK,
            edge: EDGE,
        };

        pub const LIGHT: Kinds = Kinds {
            front: super::Visual::LIGHT.text,
            family: Color::rgb(110, 110, 120),
            route: super::Visual::LIGHT.accent,
            answer: Color::rgb(0, 128, 116),
            knowledge: Color::rgb(58, 128, 30),
            model: Color::rgb(90, 104, 128),
            coder: Color::rgb(120, 70, 200),
            engine: Color::rgb(192, 50, 120),
            plugin: Color::rgb(196, 100, 0),
            screen: Color::rgb(150, 120, 0),
            gap: Color::rgb(210, 40, 40),
            weak: Color::rgb(220, 60, 60),
            edge: Color {
                red: 13,
                green: 13,
                blue: 13,
                alpha: 51,
            },
        };
    }

    /// The map's colors in the scheme the app paints with.
    #[must_use]
    pub fn current() -> &'static Kinds {
        match super::scheme() {
            super::Scheme::Light => &Kinds::LIGHT,
            super::Scheme::Dark => &Kinds::DARK,
        }
    }
}
