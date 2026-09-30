//! Main chat values reimplemented from Zeron's public dark theme and components.
//! Reference: zeronsh/zeron 50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4 (MIT).
use rust_native::layout::{Metrics, display::ColorRole};
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
pub const TRANSCRIPT: Metrics = Metrics {
    reading_width: 736,
    body_size: 14,
    body_line_height: 22,
    row_gap: 16,
    bubble_padding: 16,
    bubble_max_percent: 80,
    bubble_radius: 16,
    bubble_tail_radius: 16,
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
