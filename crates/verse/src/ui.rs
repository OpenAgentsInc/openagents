//! Screen-space UI: an amber monospace glyph atlas and a batch of quads.
//!
//! Text is Fira Mono Medium (SIL Open Font License 1.1, `assets/`),
//! rasterized once at startup into a single-channel atlas. A [`UiBatch`]
//! collects solid rectangles, frames, and text in physical pixels with the
//! origin at the top-left; the renderer draws it over the world with alpha
//! blending. Every color is a step of the amber ladder or the near-black
//! field, as in the rest of Verse.

use bytemuck::{Pod, Zeroable};
use coder_ui::theme::Intensity;

use crate::palette;

const FONT: &[u8] = include_bytes!("../assets/FiraMono-Medium.ttf");
const FIRST: u32 = 32;
const LAST: u32 = 126;
/// Characters outside printable ASCII that the atlas also carries.
const EXTRA: [char; 4] = ['·', '—', '…', '•'];
/// Latin-1 letters and punctuation, for accented names and notes.
const LATIN1: std::ops::RangeInclusive<u32> = 0xA1..=0xFF;
const ATLAS_WIDTH: u32 = 1024;

/// True when the atlas can draw `c`.
#[must_use]
pub fn drawable(c: char) -> bool {
    let n = c as u32;
    (FIRST..=LAST).contains(&n) || LATIN1.contains(&n) || EXTRA.contains(&c)
}

/// One UI vertex: pixel position, atlas coordinate, linear RGBA.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct UiVertex {
    /// Position in physical pixels, origin top-left.
    pub pos: [f32; 2],
    /// Atlas texture coordinate.
    pub uv: [f32; 2],
    /// Linear color and opacity.
    pub color: [f32; 4],
}

/// A rasterized glyph's size and placement against the pen and baseline.
#[derive(Clone, Copy, Debug, Default)]
struct Metrics {
    width: usize,
    height: usize,
    left: i32,
    top: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Glyph {
    uv0: [f32; 2],
    uv1: [f32; 2],
    size: [f32; 2],
    /// Left bearing and distance from the baseline to the bitmap's top.
    offset: [f32; 2],
}

/// The rasterized font.
pub struct Atlas {
    /// Atlas width in pixels.
    pub width: u32,
    /// Atlas height in pixels.
    pub height: u32,
    /// One coverage byte per pixel.
    pub pixels: Vec<u8>,
    glyphs: Vec<(char, Glyph)>,
    /// Horizontal advance of every glyph, in pixels.
    pub advance: f32,
    /// Line height in pixels.
    pub line: f32,
    /// Baseline distance from a line's top, in pixels.
    pub ascent: f32,
    solid: [f32; 2],
}

impl Atlas {
    /// Reuses this atlas's UVs with logical-pixel metrics. The returned layout
    /// has no bitmap and must not be uploaded as a renderer atlas.
    #[must_use]
    pub fn layout_at_scale(&self, scale: f32) -> Option<Self> {
        if !scale.is_finite() || !(0.25..=8.0).contains(&scale) {
            return None;
        }
        Some(Self {
            width: self.width,
            height: self.height,
            pixels: Vec::new(),
            glyphs: self
                .glyphs
                .iter()
                .map(|(character, glyph)| {
                    (
                        *character,
                        Glyph {
                            size: glyph.size.map(|value| value / scale),
                            offset: glyph.offset.map(|value| value / scale),
                            ..*glyph
                        },
                    )
                })
                .collect(),
            advance: self.advance / scale,
            line: self.line / scale,
            ascent: self.ascent / scale,
            solid: self.solid,
        })
    }

    /// Rasterizes the font at `px` pixels.
    ///
    /// # Panics
    ///
    /// Panics if the embedded font cannot be parsed, which a test rules out.
    #[must_use]
    pub fn new(px: f32) -> Self {
        use swash::scale::{Render, ScaleContext, Source};
        use swash::zeno::Format;
        let font = swash::FontRef::from_index(FONT, 0).expect("the embedded font parses");
        let lines = font.metrics(&[]).scale(px);
        let glyph_metrics = font.glyph_metrics(&[]).scale(px);
        let mut context = ScaleContext::new();
        let mut scaler = context.builder(font).size(px).hint(true).build();
        let chars: Vec<char> = (FIRST..=LAST)
            .chain(LATIN1)
            .filter_map(char::from_u32)
            .chain(EXTRA)
            .collect();
        let rendered: Vec<(char, Metrics, Vec<u8>)> = chars
            .iter()
            .map(|&c| {
                let id = font.charmap().map(c);
                let image = Render::new(&[Source::Outline])
                    .format(Format::Alpha)
                    .render(&mut scaler, id);
                match image {
                    Some(image) => (
                        c,
                        Metrics {
                            width: image.placement.width as usize,
                            height: image.placement.height as usize,
                            left: image.placement.left,
                            top: image.placement.top,
                        },
                        image.data,
                    ),
                    None => (c, Metrics::default(), Vec::new()),
                }
            })
            .collect();
        let advance = glyph_metrics.advance_width(font.charmap().map('M')).ceil();

        // Shelf-pack rows, leaving a solid 4x4 block at the origin.
        let pad = 1;
        let (mut x, mut y, mut row) = (4 + pad, 0u32, 4u32);
        let mut placed = Vec::with_capacity(rendered.len());
        for (c, m, bitmap) in &rendered {
            let (w, h) = (m.width as u32, m.height as u32);
            if x + w + pad > ATLAS_WIDTH {
                x = 0;
                y += row + pad;
                row = 0;
            }
            placed.push((*c, *m, x, y));
            x += w + pad;
            row = row.max(h);
            let _ = bitmap;
        }
        let height = (y + row + pad).next_power_of_two();
        let mut pixels = vec![0u8; (ATLAS_WIDTH * height) as usize];
        for py in 0..4 {
            for px_ in 0..4 {
                pixels[(py * ATLAS_WIDTH + px_) as usize] = 255;
            }
        }
        let mut glyphs = Vec::with_capacity(placed.len());
        for ((c, m, gx, gy), (_, _, bitmap)) in placed.iter().zip(&rendered) {
            if bitmap.len() < m.width * m.height {
                continue;
            }
            for row in 0..m.height {
                let src = row * m.width;
                let dst = ((gy + row as u32) * ATLAS_WIDTH + gx) as usize;
                pixels[dst..dst + m.width].copy_from_slice(&bitmap[src..src + m.width]);
            }
            let (w, h) = (ATLAS_WIDTH as f32, height as f32);
            glyphs.push((
                *c,
                Glyph {
                    uv0: [*gx as f32 / w, *gy as f32 / h],
                    uv1: [
                        (*gx as f32 + m.width as f32) / w,
                        (*gy as f32 + m.height as f32) / h,
                    ],
                    size: [m.width as f32, m.height as f32],
                    offset: [m.left as f32, m.top as f32],
                },
            ));
        }
        Self {
            width: ATLAS_WIDTH,
            height,
            pixels,
            glyphs,
            advance,
            line: (lines.ascent + lines.descent + lines.leading).ceil(),
            ascent: lines.ascent.ceil(),
            solid: [2.0 / ATLAS_WIDTH as f32, 2.0 / height as f32],
        }
    }

    fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.iter().find(|(g, _)| *g == c).map(|(_, g)| g)
    }

    /// Width of `text` in pixels.
    #[must_use]
    pub fn measure(&self, text: &str) -> f32 {
        text.chars().count() as f32 * self.advance
    }

    /// Splits `text` into lines no wider than `width` pixels, breaking at
    /// spaces where it can.
    #[must_use]
    pub fn wrap(&self, text: &str, width: f32) -> Vec<String> {
        let per = ((width / self.advance).floor() as usize).max(1);
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ') {
            let mut word = word.to_owned();
            loop {
                let len = line.chars().count();
                let need = if len == 0 { 0 } else { 1 } + word.chars().count();
                if len + need <= per {
                    if len > 0 {
                        line.push(' ');
                    }
                    line.push_str(&word);
                    break;
                }
                if len > 0 {
                    lines.push(std::mem::take(&mut line));
                    continue;
                }
                let head: String = word.chars().take(per).collect();
                word = word.chars().skip(per).collect();
                lines.push(head);
                if word.is_empty() {
                    break;
                }
            }
        }
        if !line.is_empty() || lines.is_empty() {
            lines.push(line);
        }
        lines
    }
}

/// A linear RGBA color: an amber step at an opacity.
#[must_use]
pub fn amber(step: Intensity, alpha: f32) -> [f32; 4] {
    let [r, g, b] = palette::amber(step);
    [r, g, b, alpha]
}

/// The near-black field at an opacity.
#[must_use]
pub fn field(alpha: f32) -> [f32; 4] {
    let [r, g, b] = palette::field();
    [r, g, b, alpha]
}

/// Segments in a drawn circle: smooth at the sizes the HUD draws.
const CIRCLE_SEGMENTS: usize = 64;

/// Consecutive point pairs around a circle of radius `r` at the origin.
fn circle_edges(r: f32) -> impl Iterator<Item = [[f32; 2]; 2]> {
    let point = move |i: usize| {
        let a = i as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
        [r * a.cos(), r * a.sin()]
    };
    (0..CIRCLE_SEGMENTS).map(move |i| [point(i), point(i + 1)])
}

/// Quads to draw over the world this frame.
#[derive(Clone, Debug, Default)]
pub struct UiBatch {
    /// Triangle-list vertices.
    pub vertices: Vec<UiVertex>,
}

impl UiBatch {
    /// Recolors every vertex in the neutral palette, keeping opacity: each
    /// amber step becomes its lightness in white light
    /// ([`crate::palette::neutral`]).
    pub fn neutralize(&mut self) {
        for vertex in &mut self.vertices {
            let [r, _, _, a] = vertex.color;
            vertex.color = [r, r, r, a];
        }
    }

    fn quad(&mut self, p0: [f32; 2], p1: [f32; 2], uv0: [f32; 2], uv1: [f32; 2], color: [f32; 4]) {
        let v = |x: f32, y: f32, u: f32, w: f32| UiVertex {
            pos: [x, y],
            uv: [u, w],
            color,
        };
        let a = v(p0[0], p0[1], uv0[0], uv0[1]);
        let b = v(p1[0], p0[1], uv1[0], uv0[1]);
        let c = v(p1[0], p1[1], uv1[0], uv1[1]);
        let d = v(p0[0], p1[1], uv0[0], uv1[1]);
        self.vertices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    /// A filled rectangle.
    pub fn rect(&mut self, atlas: &Atlas, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad([x, y], [x + w, y + h], atlas.solid, atlas.solid, color);
    }

    /// A rectangle outline `t` pixels thick.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        atlas: &Atlas,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        t: f32,
        color: [f32; 4],
    ) {
        self.rect(atlas, x, y, w, t, color);
        self.rect(atlas, x, y + h - t, w, t, color);
        self.rect(atlas, x, y, t, h, color);
        self.rect(atlas, x + w - t, y, t, h, color);
    }

    /// A filled circle centered at `(cx, cy)`.
    pub fn disc(&mut self, atlas: &Atlas, cx: f32, cy: f32, r: f32, color: [f32; 4]) {
        let v = |x: f32, y: f32| UiVertex {
            pos: [x, y],
            uv: atlas.solid,
            color,
        };
        for [a, b] in circle_edges(r) {
            self.vertices.extend_from_slice(&[
                v(cx, cy),
                v(cx + a[0], cy + a[1]),
                v(cx + b[0], cy + b[1]),
            ]);
        }
    }

    /// A circle outline of radius `r`, `t` pixels thick inward.
    #[allow(clippy::too_many_arguments)]
    pub fn ring(&mut self, atlas: &Atlas, cx: f32, cy: f32, r: f32, t: f32, color: [f32; 4]) {
        let v = |x: f32, y: f32| UiVertex {
            pos: [x, y],
            uv: atlas.solid,
            color,
        };
        let k = ((r - t) / r).max(0.0);
        for [a, b] in circle_edges(r) {
            let (oa, ob) = (v(cx + a[0], cy + a[1]), v(cx + b[0], cy + b[1]));
            let ia = v(cx + a[0] * k, cy + a[1] * k);
            let ib = v(cx + b[0] * k, cy + b[1] * k);
            self.vertices.extend_from_slice(&[oa, ob, ib, oa, ib, ia]);
        }
    }

    /// One line of text with its top-left at `(x, y)`. Returns its width.
    pub fn text(&mut self, atlas: &Atlas, x: f32, y: f32, text: &str, color: [f32; 4]) -> f32 {
        let baseline = (y + atlas.ascent).round();
        let mut pen = x.round();
        for c in text.chars() {
            if let Some(g) = atlas.glyph(c).or_else(|| atlas.glyph('?'))
                && g.size[0] > 0.0
            {
                let x0 = pen + g.offset[0];
                let y0 = baseline - g.offset[1];
                self.quad(
                    [x0, y0],
                    [x0 + g.size[0], y0 + g.size[1]],
                    g.uv0,
                    g.uv1,
                    color,
                );
            }
            pen += atlas.advance;
        }
        pen - x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_layout_preserves_uvs_without_copying_the_bitmap() {
        let atlas = Atlas::new(28.0);
        let layout = atlas.layout_at_scale(2.0).unwrap();
        assert!(layout.pixels.is_empty());
        assert_eq!(layout.advance * 2.0, atlas.advance);
        let source = atlas.glyph('M').unwrap();
        let logical = layout.glyph('M').unwrap();
        assert_eq!(source.uv0, logical.uv0);
        assert_eq!(source.uv1, logical.uv1);
        assert_eq!(source.size, logical.size.map(|value| value * 2.0));
        assert!(atlas.layout_at_scale(f32::NAN).is_none());
        assert!(atlas.layout_at_scale(0.0).is_none());
    }

    #[test]
    fn the_font_rasterizes_printable_ascii() {
        let atlas = Atlas::new(16.0);
        assert!(atlas.advance > 5.0);
        assert!(atlas.line >= atlas.ascent);
        for c in (FIRST..=LAST).filter_map(char::from_u32) {
            assert!(atlas.glyph(c).is_some(), "{c:?} missing");
        }
        assert!(atlas.pixels.iter().any(|&p| p > 200));
    }

    #[test]
    fn text_is_monospaced() {
        let atlas = Atlas::new(16.0);
        let mut batch = UiBatch::default();
        let w = batch.text(&atlas, 0.0, 0.0, "abc", amber(Intensity::Full, 1.0));
        assert!((w - atlas.measure("abc")).abs() < 1e-3);
        assert_eq!(batch.vertices.len(), 18);
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_splits_long_words() {
        let atlas = Atlas::new(16.0);
        let width = atlas.advance * 10.0;
        let lines = atlas.wrap("hello there general kenobi", width);
        assert_eq!(lines, ["hello", "there", "general", "kenobi"]);
        let lines = atlas.wrap("abcdefghijklmnopqrstuvwxyz", width);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.chars().count() <= 10));
    }
}
