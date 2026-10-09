//! One Rust Native node, laid out by the shared row layout and painted by
//! the transcript painter, anywhere in a frame and at any size.
//!
//! A transcript row can hold `Text`, `Markdown` (headings, lists, tables,
//! quotes, code, and inline bold, italic, and code), and `Stack` cards.
//! [`Rich`] lays one such node out with
//! [`rust_native::layout::TranscriptLayout`], the layout the desktop and
//! mobile chats use, and paints it with [`Transcript::paint`], so the
//! outlines, colors, and spacing are the chat's own. It is for content that
//! is not a scrolling conversation: a slide, a card, a help page.
//!
//! A node is laid out once at a width in points and can then be painted
//! at any pixel scale; `magnification` makes the whole block larger, as a
//! slide title or a large number is, without a second type ladder.

use crate::text::Fonts;
use crate::transcript::Transcript;
use crate::{Frame, PxRect};
use rust_native::Node;
use rust_native::layout::display::{ColorRole, FontFamily};
use rust_native::layout::shape::ShapingMeasurer;
use rust_native::layout::{
    EDGE_INSET, LayoutError, READING_WIDTH, TranscriptLayout, Update, content_band,
};
use rust_native::style::Color;

/// A laid-out node, ready to paint.
pub struct Rich {
    transcript: Transcript,
    /// The width the node was laid out at, in points.
    width: f32,
    /// Where what is drawn starts and ends, in points from the left edge.
    inset: f32,
    natural: f32,
    height: f32,
    /// The side margin the row layout added, in points.
    side: f32,
    /// Whether any part of the node scrolls sideways because it is too wide.
    scrolls: bool,
}

impl Rich {
    /// Lays `node` out `width` points wide. The row layout's reading width,
    /// [`READING_WIDTH`], is the widest a node grows; a wider `width` is
    /// laid out at the reading width.
    pub fn new(node: Node<()>, width: f32) -> Result<Rich, LayoutError> {
        Rich::in_family(node, width, FontFamily::default())
    }

    /// As [`Rich::new`], in the bundled font pair `family`, such as a
    /// [`crate::Theme`]'s `font_family`.
    pub fn in_family(node: Node<()>, width: f32, family: FontFamily) -> Result<Rich, LayoutError> {
        let width = width.clamp(1.0, READING_WIDTH);
        // The row layout keeps a side margin inside the width it gets; ask
        // for that much more, so the content band is `width`.
        let outer = width + 2.0 * content_band(width + 32.0).0;
        let mut layout = TranscriptLayout::new();
        layout.set_font_family(family);
        layout.update(
            Update {
                width: outer,
                scale: 1.0,
                order: Some(vec![node.key.clone()]),
                rows: vec![node.clone()],
                ..Update::default()
            },
            &mut ShapingMeasurer::new(),
        )?;
        let frame = layout.frame();
        let side = content_band(outer).0;
        let (inset, natural, height, scrolls) =
            frame.display(0).map_or((0.0, 0.0, 0.0, false), |row| {
                let starts = row.runs.iter().map(|run| run.x);
                let rect_starts = row.rects.iter().map(|rect| rect.x);
                let left = starts.chain(rect_starts).fold(f32::MAX, f32::min);
                let ends = row.runs.iter().map(|run| run.x + run.width);
                let rect_ends = row.rects.iter().map(|rect| rect.x + rect.w);
                let right = ends.chain(rect_ends).fold(side, f32::max);
                let left = if left == f32::MAX { side } else { left };
                (
                    (left - side).max(0.0),
                    right - side,
                    row.height,
                    !row.scrollers.is_empty(),
                )
            });
        let mut transcript = Transcript::default();
        transcript.set_font_family(family);
        transcript.update(vec![node], outer, height + 2.0 * EDGE_INSET)?;
        Ok(Rich {
            transcript,
            width,
            inset: inset.min(width),
            natural: natural.min(width),
            height,
            side,
            scrolls,
        })
    }

    /// Paints with `colors` in place of the default transcript palette,
    /// such as an app's light look.
    pub fn set_palette(&mut self, colors: &[(ColorRole, Color)]) {
        self.transcript.set_palette(colors);
    }

    /// Paints code in `palette` in place of the default syntax colors.
    pub fn set_syntax_palette(&mut self, palette: rust_native::syntax::Palette) {
        self.transcript.set_syntax_palette(palette);
    }

    /// The width the node was laid out at, in points.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Where the widest line or box ends, in points from the left edge:
    /// less than [`Rich::width`] for a short line, which is how a caller
    /// fits a box to its words.
    pub fn natural_width(&self) -> f32 {
        self.natural
    }

    /// Where the first thing drawn starts, in points from the left edge:
    /// more than zero for centered or end-aligned text.
    pub fn inset(&self) -> f32 {
        self.inset
    }

    /// The node's height, in points.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Whether a table or a code block was too wide and scrolls sideways.
    pub fn scrolls(&self) -> bool {
        self.scrolls
    }

    /// Paints the node with its top-left corner at `x`, `y` pixels, at
    /// `scale` pixels a point times `magnification`. Nothing is drawn below
    /// `clip_bottom` pixels, when given.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        frame: &mut Frame,
        fonts: &mut Fonts,
        x: f32,
        y: f32,
        scale: f32,
        magnification: f32,
        clip_bottom: Option<f32>,
    ) {
        let scale = scale * magnification;
        let bottom = y + self.height * scale;
        let rect = PxRect {
            x: x - self.side * scale,
            y: y - EDGE_INSET * scale,
            w: (self.width + 2.0 * self.side) * scale,
            h: clip_bottom.map_or(bottom, |clip| clip.min(bottom)) - (y - EDGE_INSET * scale),
        };
        if rect.h > 0.0 {
            self.transcript.paint(frame, rect, scale, fonts);
        }
    }
}

/// The transcript palette's color for `role`, so a caller's own fills and
/// rules match the painted rows.
pub fn color(role: ColorRole) -> Color {
    crate::transcript::ink(rust_native::layout::display::Ink::Role(role))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::markdown::parse;
    use rust_native::style::Style;
    use rust_native::{Element, TextRole};

    fn text(key: &str, value: &str, role: TextRole) -> Node<()> {
        Node {
            key: key.into(),
            style: Style::default(),
            element: Element::Text {
                value: value.into(),
                role,
            },
        }
    }

    #[test]
    fn a_short_line_is_narrower_than_its_width() {
        let rich = Rich::new(text("t", "Short", TextRole::Heading), 600.0).unwrap();
        assert!(rich.natural_width() > 10.0 && rich.natural_width() < 100.0);
        assert!(rich.height() > 10.0 && rich.height() < 40.0);
        assert!(!rich.scrolls());
    }

    #[test]
    fn a_wider_width_is_laid_out_at_the_reading_width() {
        let rich = Rich::new(text("t", "Short", TextRole::Body), 2_000.0).unwrap();
        assert_eq!(rich.width(), READING_WIDTH);
    }

    #[test]
    fn markdown_paints_inside_its_box() {
        let node = Node {
            key: "m".into(),
            style: Style::default(),
            element: Element::Markdown {
                blocks: parse("- **Bold.** and `code`\n- another item"),
            },
        };
        let rich = Rich::new(node, 300.0).unwrap();
        let background = Color::rgb(0, 0, 0);
        let mut frame = Frame::new(400, 200, background);
        let mut fonts = Fonts::new();
        rich.paint(&mut frame, &mut fonts, 50.0, 20.0, 1.0, 1.0, None);
        let lit = |x0: usize, x1: usize, y0: usize, y1: usize| {
            (y0..y1).any(|y| (x0..x1).any(|x| frame.pixels[(y * frame.width + x) * 4] > 60))
        };
        assert!(lit(50, 350, 20, 20 + rich.height() as usize));
        // Nothing left of the box, and nothing above it.
        assert!(!lit(0, 48, 0, 200));
        assert!(!lit(0, 400, 0, 18));
    }
}
