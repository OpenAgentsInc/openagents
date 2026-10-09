//! Main chat values reimplemented from Zeron's public dark theme and components,
//! and their Coder Light counterparts ([`Visual`]).
//! Reference: zeronsh/zeron 50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4 (MIT).
use rust_native::layout::{InlineCodeMetrics, MarkdownMetrics, Metrics, display::ColorRole};
use rust_native::style::Color;
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
pub const TRANSCRIPT: Metrics = Metrics {
    reading_width: 736,
    body_size: 14,
    body_line_height: 22,
    row_gap: 16,
    bubble_padding: 16,
    bubble_max_percent: 80,
    bubble_radius: 16,
    bubble_tail_radius: 16,
    markdown: Some(MarkdownMetrics {
        headings: [[19, 27], [16, 24], [15, 22], [14, 22]],
        code_size_half_points: 25,
        code_line_height: 18,
        code_header_height: 28,
        code_label_size: 11,
        code_padding_y: 10,
        copy_icon: true,
        strong_weight: Some(rust_native::layout::display::Weight::Semibold),
        inline_code: Some(InlineCodeMetrics {
            size_percent: 100,
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

/// Sets the scheme every view reading [`current`] paints with. The app
/// resolves it from the person's [`ThemeChoice`] and the system appearance,
/// then rebuilds its views.
pub fn set_scheme(scheme: Scheme) {
    LIGHT_ACTIVE.store(scheme == Scheme::Light, Ordering::Relaxed);
}

/// The scheme set by [`set_scheme`].
#[must_use]
pub fn scheme() -> Scheme {
    if LIGHT_ACTIVE.load(Ordering::Relaxed) {
        Scheme::Light
    } else {
        Scheme::Dark
    }
}

/// The look for the scheme the app paints with.
#[must_use]
pub fn current() -> &'static Visual {
    Visual::of(scheme())
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
            assert_eq!(style.font.size, 14.0 * scale);
            assert_eq!(style.ink, Ink::Rgba([124, 134, 255, 255]));
            let wash = row
                .rects
                .iter()
                .find(|rect| rect.fill == Some(Ink::Role(ColorRole::InlineCode)))
                .unwrap();
            assert_eq!(wash.h, 18.0 * scale);
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
        for size in [19.0, 16.0, 15.0, 14.0] {
            assert!(
                row.styles
                    .iter()
                    .any(|s| s.font.size == size && s.font.weight == Weight::Semibold)
            );
        }
        assert!(
            row.styles
                .iter()
                .any(|s| s.font.size == 12.5 && s.font.mono)
        );
        let code = row
            .rects
            .iter()
            .find(|r| r.fill == Some(Ink::Role(ColorRole::Surface)))
            .unwrap();
        assert_eq!(code.h, 68.0);
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
        assert!(((bubble.h - 20.0) % 22.0).abs() < 0.01);
        assert!(
            row.styles
                .iter()
                .all(|style| style.font.size == 14.0 && style.font.family == FontFamily::PaperMono)
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
}
