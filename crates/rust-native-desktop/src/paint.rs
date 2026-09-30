//! Paints a laid-out [`Scene`] into a [`Frame`].

use crate::canvas::{Frame, PxRect};
use crate::layout::{Op, Rect, Scene};
use crate::text::Fonts;
use rust_native::style::Color;

/// The application's painter for its drawing surfaces.
pub type SurfacePainter<'a> = dyn FnMut(&str, &mut Frame, PxRect) + 'a;

/// Retains the foreground and repaints only regions whose drawing changed.
#[derive(Default)]
pub struct Retained {
    frame: Option<Frame>,
    ops: Vec<Op>,
    transform: Option<(f32, f32, Option<Color>)>,
}

impl Retained {
    /// The most recently painted frame.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// Updates a frame and returns the pixel regions to upload. Application
    /// surfaces are refreshed even when their semantic resource stays the same.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        scene: &Scene,
        size: (usize, usize),
        scale: f32,
        scroll: f32,
        background: Option<Color>,
        fonts: &mut Fonts,
        surfaces: &mut SurfacePainter<'_>,
    ) -> Vec<PxRect> {
        let window = PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        };
        let full = self
            .frame
            .as_ref()
            .is_none_or(|frame| (frame.width, frame.height) != size)
            || self.transform != Some((scale, scroll, background))
            || self.ops.len() != scene.ops.len()
            || self.ops.iter().zip(&scene.ops).any(|(old, new)| {
                old != new
                    && (matches!(old, Op::PushClip(_) | Op::PopClip)
                        || matches!(new, Op::PushClip(_) | Op::PopClip))
            });
        if self
            .frame
            .as_ref()
            .is_none_or(|frame| (frame.width, frame.height) != size)
        {
            self.frame = Some(Frame::transparent(size.0, size.1));
        }
        let mut regions = Vec::new();
        if full {
            regions.push(window);
        } else {
            for (old, new) in self.ops.iter().zip(&scene.ops) {
                if old != new {
                    for op in [old, new] {
                        if let Some(rect) = bounds(op) {
                            add_region(
                                &mut regions,
                                pixels(rect, scale, scroll).intersection(window),
                            );
                        }
                    }
                }
                if let Op::Surface { rect, .. } = new {
                    add_region(
                        &mut regions,
                        pixels(*rect, scale, scroll).intersection(window),
                    );
                }
            }
        }
        let frame = self.frame.as_mut().expect("a retained frame");
        for region in &regions {
            frame.clear(*region, background);
            paint_clipped(scene, frame, scale, scroll, fonts, surfaces, Some(*region));
        }
        self.ops.clone_from(&scene.ops);
        self.transform = Some((scale, scroll, background));
        regions
    }
}

fn add_region(regions: &mut Vec<PxRect>, mut rect: PxRect) {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let mut index = 0;
    while index < regions.len() {
        let other = regions[index];
        let overlap = rect.intersection(other);
        if overlap.w > 0.0 && overlap.h > 0.0 {
            let x = rect.x.min(other.x);
            let y = rect.y.min(other.y);
            rect = PxRect {
                x,
                y,
                w: (rect.x + rect.w).max(other.x + other.w) - x,
                h: (rect.y + rect.h).max(other.y + other.h) - y,
            };
            regions.swap_remove(index);
            index = 0;
        } else {
            index += 1;
        }
    }
    regions.push(rect);
}

fn pixels(rect: Rect, scale: f32, scroll: f32) -> PxRect {
    let x = (rect.x * scale).floor();
    let y = ((rect.y - scroll) * scale).floor();
    PxRect {
        x,
        y,
        w: ((rect.x + rect.w) * scale).ceil() - x,
        h: ((rect.y + rect.h - scroll) * scale).ceil() - y,
    }
}

fn bounds(op: &Op) -> Option<Rect> {
    let rect = match op {
        Op::PushClip(_) | Op::PopClip => return None,
        Op::Fill { rect, .. }
        | Op::Stroke { rect, .. }
        | Op::Glyph { rect, .. }
        | Op::Check { rect, .. }
        | Op::Surface { rect, .. } => *rect,
        Op::Text {
            paragraph,
            x,
            y,
            width,
            ..
        } => Rect {
            x: *x,
            y: *y,
            w: width.max(paragraph.width),
            h: paragraph.height,
        },
    };
    // Text overhangs and antialiased edges can extend beyond logical bounds.
    Some(Rect {
        x: rect.x - 2.0,
        y: rect.y - 2.0,
        w: rect.w + 4.0,
        h: rect.h + 4.0,
    })
}

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
    paint_clipped(scene, frame, scale, scroll, fonts, surfaces, None);
}

#[allow(clippy::too_many_arguments)]
fn paint_clipped(
    scene: &Scene,
    frame: &mut Frame,
    scale: f32,
    scroll: f32,
    fonts: &mut Fonts,
    surfaces: &mut SurfacePainter<'_>,
    base_clip: Option<PxRect>,
) {
    let mut clips: Vec<PxRect> = Vec::new();
    let px = |rect: &Rect| PxRect {
        x: (rect.x * scale).round(),
        y: ((rect.y - scroll) * scale).round(),
        w: (rect.w * scale).round(),
        h: (rect.h * scale).round(),
    };
    frame.set_clip(base_clip);
    for op in &scene.ops {
        if let Some(rect) = bounds(op)
            && !frame.visible(pixels(rect, scale, scroll))
        {
            continue;
        }
        match op {
            Op::PushClip(rect) => {
                let rect = px(rect);
                let clip = frame
                    .clip()
                    .map_or(rect, |parent| parent.intersection(rect));
                clips.push(clip);
                frame.set_clip(Some(clip));
            }
            Op::PopClip => {
                clips.pop();
                frame.set_clip(clips.last().copied().or(base_clip));
            }
            Op::Glyph { rect, glyph, color } => crate::icons::draw(frame, px(rect), *glyph, *color),
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
    frame.set_clip(None);
}
