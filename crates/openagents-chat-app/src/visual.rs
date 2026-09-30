//! Main chat values reimplemented from Zeron's public dark theme and components.
//! Reference: zeronsh/zeron 50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4 (MIT).
use rust_native::layout::{MarkdownMetrics, Metrics, display::ColorRole};
use rust_native::style::Color;

pub const CANVAS: Color = Color::rgb(6, 6, 6);
pub const SIDEBAR: Color = Color::rgb(13, 13, 13);
pub const COMPOSER: Color = Color::rgb(13, 13, 13);
pub const TEXT: Color = Color::rgb(229, 229, 229);
pub const MUTED: Color = Color::rgb(163, 163, 163);
pub const FAINT: Color = Color::rgb(115, 115, 115);
pub const SELECTED: Color = Color::rgb(37, 37, 37);
pub const BORDER: Color = Color::rgb(26, 26, 26);
pub const COMPOSER_BORDER: Color = Color::rgb(29, 30, 31);
pub const COLORS: [(ColorRole, Color); 9] = [
    (ColorRole::Primary, TEXT),
    (ColorRole::Secondary, MUTED),
    (ColorRole::Tertiary, FAINT),
    (ColorRole::Link, Color::rgb(129, 140, 248)),
    (ColorRole::Bubble, Color::rgb(26, 26, 26)),
    (ColorRole::Surface, Color::rgb(14, 14, 14)),
    (ColorRole::Raised, Color::rgb(32, 32, 32)),
    (ColorRole::Border, BORDER),
    (ColorRole::InlineCode, Color::rgb(26, 26, 26)),
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
    }),
};

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
        layout.set_font_family(FontFamily::Geist);
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
        layout.set_font_family(FontFamily::Geist);
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
                .all(|style| style.font.size == 14.0 && style.font.family == FontFamily::Geist)
        );
        assert_eq!(layout.update(update(), &mut measurer).unwrap().relaid, 0);
        let mut invalid = TRANSCRIPT;
        invalid.bubble_max_percent = 101;
        assert!(layout.set_metrics(invalid).is_err());
    }
}
