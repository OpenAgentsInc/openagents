//! Small vector glyphs for the semantic button icon set.

use crate::canvas::{Frame, PxRect};
use rust_native::Glyph;
use rust_native::style::Color;

pub(crate) fn draw(frame: &mut Frame, rect: PxRect, glyph: Glyph, color: Color) {
    let p = |x: f32, y: f32| (rect.x + x * rect.w, rect.y + y * rect.h);
    let width = (rect.w / 11.0).max(1.0);
    let mut line =
        |a: (f32, f32), b: (f32, f32)| frame.line(p(a.0, a.1), p(b.0, b.1), width, color);
    match glyph {
        Glyph::Search | Glyph::Settings | Glyph::Pin | Glyph::Archive | Glyph::Restore => {
            crate::solar::draw_glyph(frame, rect, glyph, color);
        }
        Glyph::Menu => {
            line((0.12, 0.2), (0.88, 0.2));
            line((0.12, 0.5), (0.88, 0.5));
            line((0.12, 0.8), (0.88, 0.8));
        }
        Glyph::Back => {
            line((0.65, 0.15), (0.3, 0.5));
            line((0.3, 0.5), (0.65, 0.85));
        }
        Glyph::Add => {
            line((0.5, 0.15), (0.5, 0.85));
            line((0.15, 0.5), (0.85, 0.5));
        }
        Glyph::ArrowUp => {
            line((0.5, 0.85), (0.5, 0.15));
            line((0.2, 0.45), (0.5, 0.15));
            line((0.5, 0.15), (0.8, 0.45));
        }
        Glyph::Stop => frame.fill(
            PxRect {
                x: rect.x + rect.w * 0.2,
                y: rect.y + rect.h * 0.2,
                w: rect.w * 0.6,
                h: rect.h * 0.6,
            },
            rect.w * 0.1,
            color,
        ),
        Glyph::More => {
            for x in [0.15, 0.45, 0.75] {
                frame.fill(
                    PxRect {
                        x: rect.x + rect.w * x,
                        y: rect.y + rect.h * 0.45,
                        w: rect.w * 0.12,
                        h: rect.h * 0.12,
                    },
                    rect.w * 0.06,
                    color,
                );
            }
        }
        Glyph::Paperclip => {
            line((0.38, 0.68), (0.72, 0.34));
            line((0.72, 0.34), (0.6, 0.22));
            line((0.6, 0.22), (0.2, 0.62));
            line((0.2, 0.62), (0.2, 0.8));
            line((0.2, 0.8), (0.38, 0.9));
            line((0.38, 0.9), (0.9, 0.38));
            line((0.9, 0.38), (0.9, 0.18));
            line((0.9, 0.18), (0.72, 0.08));
            line((0.72, 0.08), (0.28, 0.52));
        }
        Glyph::Clipboard => {
            line((0.3, 0.2), (0.16, 0.2));
            line((0.16, 0.2), (0.16, 0.92));
            line((0.16, 0.92), (0.84, 0.92));
            line((0.84, 0.92), (0.84, 0.2));
            line((0.84, 0.2), (0.7, 0.2));
            frame.stroke(
                PxRect {
                    x: rect.x + rect.w * 0.3,
                    y: rect.y + rect.h * 0.08,
                    w: rect.w * 0.4,
                    h: rect.h * 0.24,
                },
                rect.w * 0.05,
                width,
                color,
            );
        }
        Glyph::Compose | Glyph::Edit => {
            line((0.2, 0.7), (0.72, 0.18));
            line((0.72, 0.18), (0.88, 0.34));
            line((0.88, 0.34), (0.36, 0.86));
            line((0.36, 0.86), (0.15, 0.9));
            line((0.15, 0.9), (0.2, 0.7));
            line((0.62, 0.28), (0.78, 0.44));
        }
        Glyph::Folder => {
            line((0.08, 0.25), (0.38, 0.25));
            line((0.38, 0.25), (0.5, 0.38));
            line((0.5, 0.38), (0.92, 0.38));
            line((0.92, 0.38), (0.92, 0.83));
            line((0.92, 0.83), (0.08, 0.83));
            line((0.08, 0.83), (0.08, 0.25));
        }
        Glyph::Computer => {
            line((0.12, 0.18), (0.88, 0.18));
            line((0.88, 0.18), (0.88, 0.7));
            line((0.88, 0.7), (0.12, 0.7));
            line((0.12, 0.7), (0.12, 0.18));
            line((0.5, 0.7), (0.5, 0.9));
            line((0.3, 0.9), (0.7, 0.9));
        }
        Glyph::Check | Glyph::Checked => {
            line((0.12, 0.5), (0.38, 0.76));
            line((0.38, 0.76), (0.88, 0.22));
        }
        Glyph::Flag => {
            line((0.18, 0.9), (0.18, 0.12));
            line((0.18, 0.12), (0.83, 0.2));
            line((0.83, 0.2), (0.72, 0.55));
            line((0.72, 0.55), (0.18, 0.48));
        }
        Glyph::Terminal => {
            line((0.15, 0.2), (0.45, 0.5));
            line((0.45, 0.5), (0.15, 0.8));
            line((0.55, 0.8), (0.88, 0.8));
        }
        Glyph::Ask => {
            line((0.15, 0.18), (0.85, 0.18));
            line((0.85, 0.18), (0.85, 0.68));
            line((0.85, 0.68), (0.42, 0.68));
            line((0.42, 0.68), (0.15, 0.9));
            line((0.15, 0.9), (0.15, 0.18));
        }
        Glyph::History => {
            line((0.5, 0.25), (0.5, 0.5));
            line((0.5, 0.5), (0.7, 0.6));
            frame.stroke(rect, rect.w / 2.0, width, color);
        }
        Glyph::Person => {
            frame.stroke(
                PxRect {
                    x: rect.x + rect.w * 0.3,
                    y: rect.y + rect.h * 0.08,
                    w: rect.w * 0.4,
                    h: rect.h * 0.4,
                },
                rect.w * 0.2,
                width,
                color,
            );
            frame.stroke(
                PxRect {
                    x: rect.x + rect.w * 0.08,
                    y: rect.y + rect.h * 0.58,
                    w: rect.w * 0.84,
                    h: rect.h * 0.36,
                },
                rect.w * 0.15,
                width,
                color,
            );
        }
        Glyph::Cloud => {
            frame.stroke(
                PxRect {
                    y: rect.y + rect.h * 0.35,
                    h: rect.h * 0.5,
                    ..rect
                },
                rect.w * 0.22,
                width,
                color,
            );
        }
        Glyph::Wallet => {
            frame.stroke(rect, rect.w * 0.15, width, color);
            frame.stroke(
                PxRect {
                    x: rect.x + rect.w * 0.55,
                    y: rect.y + rect.h * 0.35,
                    w: rect.w * 0.45,
                    h: rect.h * 0.3,
                },
                width,
                width,
                color,
            );
        }
        Glyph::Key => {
            line((0.4, 0.6), (0.87, 0.13));
            line((0.7, 0.3), (0.86, 0.46));
            frame.stroke(
                PxRect {
                    x: rect.x + rect.w * 0.07,
                    y: rect.y + rect.h * 0.5,
                    w: rect.w * 0.42,
                    h: rect.h * 0.42,
                },
                rect.w * 0.21,
                width,
                color,
            );
        }
        Glyph::Unchecked => frame.stroke(rect, rect.w * 0.15, width, color),
    }
}

/// Paints a bundled vector icon into a locally registered surface.
pub fn paint(
    frame: &mut Frame,
    rect: PxRect,
    glyph: Glyph,
    set: crate::theme::IconSet,
    color: Color,
) {
    if set != crate::theme::IconSet::Solar || !crate::solar::draw_glyph(frame, rect, glyph, color) {
        draw(frame, rect, glyph, color);
    }
}
