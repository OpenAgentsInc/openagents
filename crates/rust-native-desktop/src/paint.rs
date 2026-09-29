//! Paints a laid-out [`Scene`] into a [`Frame`].

use crate::canvas::{Frame, PxRect};
use crate::layout::{Op, Rect, Scene};
use crate::text::Fonts;

/// The application's painter for its drawing surfaces.
pub type SurfacePainter<'a> = dyn FnMut(&str, &mut Frame, PxRect) + 'a;

/// Paints `scene` into `frame` at `scale` pixels a point, scrolled up by
/// `scroll` points. The frame is not cleared first.
pub fn paint(
    scene: &Scene,
    frame: &mut Frame,
    scale: f32,
    scroll: f32,
    fonts: &mut Fonts,
    surfaces: &mut SurfacePainter<'_>,
) {
    let px = |rect: &Rect| PxRect {
        x: (rect.x * scale).round(),
        y: ((rect.y - scroll) * scale).round(),
        w: (rect.w * scale).round(),
        h: (rect.h * scale).round(),
    };
    for op in &scene.ops {
        match op {
            Op::Fill {
                rect,
                radius,
                color,
            } => frame.fill(px(rect), radius * scale, *color),
            Op::Stroke {
                rect,
                radius,
                width,
                color,
            } => frame.stroke(px(rect), radius * scale, width * scale, *color),
            Op::Text {
                paragraph,
                x,
                y,
                width,
                align,
                color,
            } => fonts.draw(
                frame,
                paragraph,
                (x * scale).round(),
                ((y - scroll) * scale).round(),
                *width,
                *align,
                scale,
                *color,
            ),
            Op::Check { rect, color } => {
                let r = px(rect);
                let point = |fx: f32, fy: f32| (r.x + r.w * fx, r.y + r.h * fy);
                let width = (2.0 * scale).max(1.5);
                frame.line(point(0.24, 0.52), point(0.42, 0.70), width, *color);
                frame.line(point(0.42, 0.70), point(0.76, 0.32), width, *color);
            }
            Op::Surface { resource, rect } => surfaces(resource, frame, px(rect)),
        }
    }
}
