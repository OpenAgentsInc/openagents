//! Screen-space UI: glyph atlases, optional color sprites, and batches of quads.
//!
//! Text is Fira Mono Medium (SIL Open Font License 1.1, `assets/`),
//! rasterized once at startup into a single-channel atlas. A [`UiBatch`]
//! collects solid rectangles, frames, and text in physical pixels with the
//! origin at the top-left; the renderer draws it over the world with alpha
//! blending. Every color is a step of the amber ladder or the near-black
//! field by default. Imported scenes can supply a proportional font and color sprites.

use coder_ui::theme::Intensity;
pub use verse_engine::overlay::Vertex as UiVertex;

use crate::palette;

// The font stays in `crates/verse/assets` beside its license, where the
// imported scenes read it too.
const FONT: &[u8] = include_bytes!("../../verse/assets/FiraMono-Medium.ttf");
/// Fira Mono Medium, the atlas's font, for callers that rasterize more of
/// its glyphs on demand ([`Atlas::insert_glyph`]).
pub const MONO_FONT: &[u8] = FONT;
/// The sprite that holds glyphs added after the atlas was built.
const DYNAMIC: &str = "glyphs:dynamic";
/// The WGSL shader that draws a [`UiBatch`] and an overlay panel.
pub const SHADER: &str = include_str!("ui.wgsl");
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
    advance: f32,
}

/// A glyph's box in the atlas's layout units: its bitmap size, its left
/// bearing and height above the baseline, and its advance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphBox {
    pub size: [f32; 2],
    pub offset: [f32; 2],
    pub advance: f32,
}

/// Where the next glyph goes in the reserved region, in pixels from its
/// top-left: the pen and the current shelf's height.
#[derive(Clone, Copy, Debug, Default)]
struct Pen {
    x: u32,
    y: u32,
    row: u32,
}

/// The rasterized font.
#[derive(Clone)]
pub struct Atlas {
    /// Atlas width in pixels.
    pub width: u32,
    /// Atlas height in pixels.
    pub height: u32,
    /// One coverage byte per pixel.
    pub pixels: Vec<u8>,
    pub rgba: Option<Vec<u8>>,
    pub sprites: std::collections::BTreeMap<String, ([f32; 2], [f32; 2])>,
    fonts: std::collections::BTreeMap<String, Atlas>,
    glyphs: Vec<(char, Glyph)>,
    /// Each glyph's place in `glyphs`.
    index: std::collections::HashMap<char, u32>,
    /// Changes whenever the bitmap or its layout changes, so a renderer
    /// knows to upload it again.
    revision: u64,
    /// The pixel size the bitmaps were rasterized at.
    pub raster_px: f32,
    /// Bitmap pixels per layout unit ([`Atlas::use_logical_metrics`]).
    density: f32,
    pen: Pen,
    /// Default glyph advance in pixels; proportional glyphs retain their own metrics.
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
            rgba: None,
            sprites: self.sprites.clone(),
            fonts: Default::default(),
            index: self.index.clone(),
            revision: self.revision,
            raster_px: self.raster_px,
            density: self.density * scale,
            pen: self.pen,
            glyphs: self
                .glyphs
                .iter()
                .map(|(character, glyph)| {
                    (
                        *character,
                        Glyph {
                            size: glyph.size.map(|value| value / scale),
                            offset: glyph.offset.map(|value| value / scale),
                            advance: glyph.advance / scale,
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

    /// Keep high-density glyph bitmaps while expressing layout in logical pixels.
    pub fn use_logical_metrics(&mut self, density: f32) {
        assert!(density.is_finite() && (1.0..=8.0).contains(&density));
        for (_, glyph) in &mut self.glyphs {
            glyph.size = glyph.size.map(|v| v / density);
            glyph.offset = glyph.offset.map(|v| v / density);
            glyph.advance /= density;
        }
        self.advance /= density;
        self.line /= density;
        self.ascent /= density;
        self.density *= density;
        for font in self.fonts.values_mut() {
            font.use_logical_metrics(density);
        }
    }
    /// Rasterizes the font at `px` pixels.
    ///
    /// # Panics
    ///
    /// Panics if the embedded font cannot be parsed, which a test rules out.
    #[must_use]
    pub fn new(px: f32) -> Self {
        Self::rasterize(FONT, px, false).expect("the embedded font parses")
    }
    /// Rasterizes a caller-provided proportional font without retaining its source.
    pub fn from_font(bytes: &[u8], px: f32) -> Result<Self, String> {
        if bytes.len() > 16 * 1024 * 1024 || !px.is_finite() || !(1.0..=128.0).contains(&px) {
            return Err("Invalid UI font size".into());
        }
        Self::rasterize(bytes, px, true)
    }
    fn rasterize(bytes: &[u8], px: f32, proportional: bool) -> Result<Self, String> {
        use swash::scale::{Render, ScaleContext, Source};
        use swash::zeno::Format;
        let font = swash::FontRef::from_index(bytes, 0).ok_or("Invalid UI font")?;
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
                    advance: if proportional {
                        glyph_metrics.advance_width(font.charmap().map(*c))
                    } else {
                        advance
                    },
                },
            ));
        }
        let index = glyphs
            .iter()
            .enumerate()
            .map(|(i, (c, _))| (*c, i as u32))
            .collect();
        Ok(Self {
            width: ATLAS_WIDTH,
            height,
            pixels,
            rgba: None,
            sprites: Default::default(),
            fonts: Default::default(),
            index,
            revision: 0,
            raster_px: px,
            density: 1.0,
            pen: Pen::default(),
            glyphs,
            advance,
            line: (lines.ascent + lines.descent + lines.leading).ceil(),
            ascent: lines.ascent.ceil(),
            solid: [2.0 / ATLAS_WIDTH as f32, 2.0 / height as f32],
        })
    }

    /// Adds a color sprite to the same GPU atlas as the glyphs.
    pub fn add_sprite(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), String> {
        if width > self.width
            || height == 0
            || height > 4096
            || pixels.len() != (width * height * 4) as usize
        {
            return Err("Invalid UI sprite".into());
        }
        let old = self.height;
        let row_of = |v: f32| (v * old as f32).round() as u32;
        let column_of = |u: f32| (u * self.width as f32).round() as u32;
        // Sprites pack in shelves: a sprite as tall as the last shelf goes
        // to the right of its sprites while the row has room, and any other
        // starts a shelf below.
        let bottom = self.sprites.values().map(|(_, end)| row_of(end[1])).max();
        let shelf = bottom.and_then(|bottom| {
            let on: Vec<_> = self
                .sprites
                .values()
                .filter(|(_, end)| row_of(end[1]) == bottom)
                .collect();
            let tall = on.iter().all(|(top, _)| bottom - row_of(top[1]) == height);
            let right = on.iter().map(|(_, end)| column_of(end[0])).max()?;
            (tall && right + width <= self.width).then(|| (right, bottom - height))
        });
        let (left, start) = shelf.unwrap_or((
            0,
            bottom.unwrap_or((self.pixels.len() / self.width as usize) as u32),
        ));
        let new = old.max((start + height).next_power_of_two());
        if new > 8192 {
            return Err("UI sprite atlas exceeds 8192 rows".into());
        }
        let mut rgba = self.rgba.take().unwrap_or_else(|| {
            self.pixels
                .iter()
                .flat_map(|a| [255, 255, 255, *a])
                .collect()
        });
        rgba.resize((self.width * new * 4) as usize, 0);
        for row in 0..height as usize {
            let dst = ((start as usize + row) * self.width as usize + left as usize) * 4;
            rgba[dst..dst + width as usize * 4]
                .copy_from_slice(&pixels[row * width as usize * 4..(row + 1) * width as usize * 4]);
        }
        let ratio = old as f32 / new as f32;
        for (_, g) in &mut self.glyphs {
            g.uv0[1] *= ratio;
            g.uv1[1] *= ratio;
        }
        for (a, b) in self.sprites.values_mut() {
            a[1] *= ratio;
            b[1] *= ratio;
        }
        for font in self.fonts.values_mut() {
            for (_, glyph) in &mut font.glyphs {
                glyph.uv0[1] *= ratio;
                glyph.uv1[1] *= ratio;
            }
            font.solid[1] *= ratio;
            font.height = new;
        }
        self.solid[1] *= ratio;
        self.sprites.insert(
            name.into(),
            (
                [left as f32 / self.width as f32, start as f32 / new as f32],
                [
                    (left + width) as f32 / self.width as f32,
                    (start + height) as f32 / new as f32,
                ],
            ),
        );
        self.height = new;
        self.rgba = Some(rgba);
        self.revision += 1;
        Ok(())
    }

    /// A counter that changes whenever the bitmap or its layout changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Reserves `rows` full-width rows for glyphs added later with
    /// [`Atlas::insert_glyph`]. Call it before a renderer uploads the
    /// atlas, so later glyphs only rewrite its texture.
    ///
    /// # Errors
    /// The atlas would grow past its limit.
    pub fn reserve_glyphs(&mut self, rows: u32) -> Result<(), String> {
        if self.sprites.contains_key(DYNAMIC) {
            return Ok(());
        }
        let width = self.width;
        self.add_sprite(DYNAMIC, width, rows, &vec![0; (width * rows * 4) as usize])
    }

    /// Whether the atlas has a glyph for `c`.
    #[must_use]
    pub fn has_glyph(&self, c: char) -> bool {
        self.index.contains_key(&c)
    }

    /// `c`'s box, when the atlas has it.
    #[must_use]
    pub fn glyph_box(&self, c: char) -> Option<GlyphBox> {
        self.glyph(c).map(|g| GlyphBox {
            size: g.size,
            offset: g.offset,
            advance: g.advance,
        })
    }

    /// Adds `c` from a coverage bitmap `width` by `height` pixels, rasterized
    /// at [`Atlas::raster_px`], with its left bearing, its top above the
    /// baseline, and its advance in those pixels. It goes into the rows
    /// [`Atlas::reserve_glyphs`] kept. Returns false when there are none,
    /// they are full, or the bitmap does not match its size.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_glyph(
        &mut self,
        c: char,
        coverage: &[u8],
        width: u32,
        height: u32,
        left: i32,
        top: i32,
        advance: f32,
    ) -> bool {
        if self.has_glyph(c) {
            return true;
        }
        let Some((a, b)) = self.sprites.get(DYNAMIC).copied() else {
            return false;
        };
        if coverage.len() != (width * height) as usize {
            return false;
        }
        let (atlas_w, atlas_h) = (self.width as f32, self.height as f32);
        let x0 = (a[0] * atlas_w).round() as u32;
        let y0 = (a[1] * atlas_h).round() as u32;
        let region_w = ((b[0] - a[0]) * atlas_w).round() as u32;
        let region_h = ((b[1] - a[1]) * atlas_h).round() as u32;
        let pad = 1;
        let mut pen = self.pen;
        if pen.x + width + pad > region_w {
            pen.x = 0;
            pen.y += pen.row + pad;
            pen.row = 0;
        }
        if width + pad > region_w || pen.y + height > region_h {
            return false;
        }
        let Some(rgba) = self.rgba.as_mut() else {
            return false;
        };
        let (gx, gy) = (x0 + pen.x, y0 + pen.y);
        for row in 0..height {
            for col in 0..width {
                let at = (((gy + row) * self.width + gx + col) * 4) as usize;
                rgba[at..at + 4].copy_from_slice(&[
                    255,
                    255,
                    255,
                    coverage[(row * width + col) as usize],
                ]);
            }
        }
        pen.x += width + pad;
        pen.row = pen.row.max(height);
        self.pen = pen;
        let d = self.density;
        self.glyphs.push((
            c,
            Glyph {
                uv0: [gx as f32 / atlas_w, gy as f32 / atlas_h],
                uv1: [
                    (gx + width) as f32 / atlas_w,
                    (gy + height) as f32 / atlas_h,
                ],
                size: [width as f32 / d, height as f32 / d],
                offset: [left as f32 / d, top as f32 / d],
                advance: advance / d,
            },
        ));
        self.index.insert(c, (self.glyphs.len() - 1) as u32);
        self.revision += 1;
        true
    }

    /// Packs another font into this atlas while retaining its own metrics.
    pub fn add_font(&mut self, name: &str, bytes: &[u8], px: f32) -> Result<(), String> {
        let mut font = Self::from_font(bytes, px)?;
        let rgba: Vec<u8> = font
            .pixels
            .iter()
            .flat_map(|a| [255, 255, 255, *a])
            .collect();
        let sprite = format!("font:{name}");
        self.add_sprite(&sprite, font.width, font.height, &rgba)?;
        let (a, b) = self.sprites[&sprite];
        for (_, g) in &mut font.glyphs {
            for uv in [&mut g.uv0, &mut g.uv1] {
                uv[0] = a[0] + uv[0] * (b[0] - a[0]);
                uv[1] = a[1] + uv[1] * (b[1] - a[1]);
            }
        }
        font.solid = self.solid;
        font.width = self.width;
        font.height = self.height;
        font.pixels.clear();
        self.fonts.insert(name.into(), font);
        Ok(())
    }
    /// Returns a named font layout, falling back to the primary font.
    pub fn font(&self, name: &str) -> &Self {
        self.fonts.get(name).unwrap_or(self)
    }

    fn glyph(&self, c: char) -> Option<&Glyph> {
        self.index
            .get(&c)
            .and_then(|&i| self.glyphs.get(i as usize))
            .map(|(_, g)| g)
    }

    /// Width of `text` in pixels.
    #[must_use]
    pub fn measure(&self, text: &str) -> f32 {
        text.chars()
            .map(|c| self.glyph(c).map_or(self.advance, |g| g.advance))
            .sum()
    }

    /// Splits `text` into lines no wider than `width` pixels, breaking at
    /// spaces where it can.
    #[must_use]
    pub fn wrap(&self, text: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ') {
            let mut word = word.to_owned();
            loop {
                let len = line.chars().count();
                let candidate = if len == 0 {
                    word.clone()
                } else {
                    format!("{line} {word}")
                };
                if self.measure(&candidate) <= width {
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
                let mut per = 0;
                let mut used = 0.0;
                for c in word.chars() {
                    let advance = self.glyph(c).map_or(self.advance, |g| g.advance);
                    if per > 0 && used + advance > width {
                        break;
                    }
                    per += 1;
                    used += advance;
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

    /// A filled triangle.
    pub fn triangle(
        &mut self,
        atlas: &Atlas,
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        color: [f32; 4],
    ) {
        let v = |pos: [f32; 2]| UiVertex {
            pos,
            uv: atlas.solid,
            color,
        };
        self.vertices.extend_from_slice(&[v(a), v(b), v(c)]);
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

    pub fn image(
        &mut self,
        atlas: &Atlas,
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        if let Some((a, b)) = atlas.sprites.get(name) {
            self.quad([x, y], [x + w, y + h], *a, *b, color);
        }
    }

    /// Draws a normalized crop of a sprite; reversed coordinates mirror it.
    pub fn image_region(
        &mut self,
        atlas: &Atlas,
        name: &str,
        rect: [f32; 4],
        uv: [f32; 4],
        color: [f32; 4],
    ) {
        if let Some((a, b)) = atlas.sprites.get(name) {
            let map = |u: f32, v: f32| [a[0] + u * (b[0] - a[0]), a[1] + v * (b[1] - a[1])];
            self.quad(
                [rect[0], rect[1]],
                [rect[0] + rect[2], rect[1] + rect[3]],
                map(uv[0], uv[2]),
                map(uv[1], uv[3]),
                color,
            );
        }
    }
    /// Draws the remaining clockwise cooldown sector inside a square icon.
    pub fn cooldown(&mut self, atlas: &Atlas, rect: [f32; 3], remaining: f32) {
        let remaining = remaining.clamp(0.0, 1.0);
        if remaining == 0.0 {
            return;
        }
        let start = (1.0 - remaining) * std::f32::consts::TAU;
        let mut angles = vec![start];
        for corner in [0.25, 0.75, 1.25, 1.75] {
            let angle = corner * std::f32::consts::PI;
            if angle > start {
                angles.push(angle);
            }
        }
        angles.push(std::f32::consts::TAU);
        let center = [rect[0] + rect[2] * 0.5, rect[1] + rect[2] * 0.5];
        let vertex = |p| UiVertex {
            pos: p,
            uv: atlas.solid,
            color: [0.0, 0.0, 0.0, 0.65],
        };
        let edge = |angle: f32| {
            let (s, c) = angle.sin_cos();
            let r = rect[2] * 0.5 / s.abs().max(c.abs());
            [center[0] + s * r, center[1] - c * r]
        };
        for pair in angles.windows(2) {
            self.vertices.extend_from_slice(&[
                vertex(center),
                vertex(edge(pair[0])),
                vertex(edge(pair[1])),
            ]);
        }
    }

    /// A straight stroke from `a` to `b`, `w` pixels wide.
    pub fn line(&mut self, atlas: &Atlas, a: [f32; 2], b: [f32; 2], w: f32, color: [f32; 4]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dy);
        if length <= f32::EPSILON {
            return;
        }
        let (nx, ny) = (-dy / length * w / 2.0, dx / length * w / 2.0);
        let v = |x: f32, y: f32| UiVertex {
            pos: [x, y],
            uv: atlas.solid,
            color,
        };
        let (p0, p1) = (v(a[0] + nx, a[1] + ny), v(b[0] + nx, b[1] + ny));
        let (p2, p3) = (v(b[0] - nx, b[1] - ny), v(a[0] - nx, a[1] - ny));
        self.vertices.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
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
            pen += atlas.glyph(c).map_or(atlas.advance, |g| g.advance);
        }
        pen - x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_geometry_uses_the_portable_overlay_contract() {
        let atlas = Atlas::new(18.0);
        let mut batch = UiBatch::default();
        batch.text(&atlas, -10.0, 20.0, "Adventurer", [1.0; 4]);
        batch.rect(&atlas, 10.0, 50.0, 128.0, 12.0, [1.0, 0.0, 0.0, 1.0]);
        batch.disc(&atlas, 200.0, 200.0, 20.0, [0.5; 4]);
        batch.ring(&atlas, 250.0, 200.0, 20.0, 2.0, [1.0; 4]);
        let pack = serde_json::from_value(serde_json::json!({
            "version":1,"source_revision":"test","textures":[],"models":{
                "room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[],"clips":[],"height":1,"attachments":[]}
            }
        })).unwrap();
        let catalog = verse_engine::residency::Catalog::new(&pack).unwrap();
        let overlay =
            verse_engine::overlay::ResolvedOverlay::extract(&catalog, &batch.vertices).unwrap();
        assert_eq!(std::mem::size_of::<UiVertex>(), 32);
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(overlay.vertices()),
            bytemuck::cast_slice::<_, u8>(&batch.vertices)
        );
    }

    #[test]
    fn color_sprites_preserve_glyph_texels_and_layout() {
        let mut atlas = Atlas::new(16.0);
        let old_height = atlas.height;
        let old_uv = atlas.glyph('A').unwrap().uv0;
        let old_coverage = atlas.pixels.clone();
        atlas
            .add_sprite("border", 2, 1, &[255, 0, 0, 255, 0, 255, 0, 128])
            .unwrap();
        let rgba = atlas.rgba.as_ref().unwrap();
        for (i, alpha) in old_coverage.iter().enumerate() {
            assert_eq!(&rgba[i * 4..i * 4 + 4], &[255, 255, 255, *alpha]);
        }
        assert_eq!(
            atlas.glyph('A').unwrap().uv0[1] * atlas.height as f32,
            old_uv[1] * old_height as f32
        );
        assert_eq!(atlas.layout_at_scale(2.0).unwrap().sprites, atlas.sprites);
        assert!(Atlas::from_font(b"invalid", 16.0).is_err());
    }

    #[test]
    fn action_icons_pack_into_existing_rows_without_exponential_growth() {
        let mut atlas = Atlas::new(18.0);
        for i in 0..7 {
            atlas
                .add_sprite(&format!("icon-{i}"), 64, 64, &vec![255; 64 * 64 * 4])
                .unwrap();
        }
        assert!(atlas.height <= 1024);
        for (_, end) in atlas.sprites.values() {
            assert!(end[1] <= 1.0);
        }
    }

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
    fn glyphs_added_later_go_into_the_reserved_rows() {
        let mut atlas = Atlas::new(16.0);
        assert!(!atlas.insert_glyph('世', &[255; 4], 2, 2, 0, 2, 16.0));
        atlas.reserve_glyphs(64).unwrap();
        let height = atlas.height;
        let revision = atlas.revision();
        let a_uv = atlas.glyph('a').unwrap().uv0;
        assert!(!atlas.has_glyph('世'));
        assert!(!atlas.insert_glyph('世', &[255; 3], 2, 2, 0, 2, 16.0));
        assert!(atlas.insert_glyph('世', &[255, 128, 64, 0], 2, 2, 1, 12, 16.0));
        assert!(atlas.has_glyph('世'));
        assert!(atlas.revision() > revision);
        // Nothing moves: the texture keeps its size and old glyphs their UVs.
        assert_eq!(atlas.height, height);
        assert_eq!(atlas.glyph('a').unwrap().uv0, a_uv);
        let glyph = atlas.glyph_box('世').unwrap();
        assert_eq!(glyph.size, [2.0, 2.0]);
        assert_eq!(glyph.offset, [1.0, 12.0]);
        let uv = atlas.glyph('世').unwrap().uv0;
        let (x, y) = (
            (uv[0] * atlas.width as f32) as usize,
            (uv[1] * atlas.height as f32) as usize,
        );
        let rgba = atlas.rgba.as_ref().unwrap();
        let at = (y * atlas.width as usize + x) * 4;
        assert_eq!(&rgba[at..at + 8], &[255, 255, 255, 255, 255, 255, 255, 128]);
        // A full region refuses more.
        let big = vec![0; 1024 * 70];
        assert!(!atlas.insert_glyph('界', &big, 1024, 70, 0, 0, 16.0));
        let mut batch = UiBatch::default();
        batch.text(&atlas, 0.0, 0.0, "a世", [1.0; 4]);
        assert_eq!(batch.vertices.len(), 12);
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

#[cfg(test)]
mod sprite_layout_tests {
    use super::*;
    #[test]
    fn additional_fonts_survive_atlas_growth_and_cropped_images_keep_uvs() {
        let mut atlas = Atlas::new(18.0);
        atlas.add_font("numbers", FONT, 12.0).unwrap();
        let glyph = atlas.font("numbers").glyph('8').unwrap();
        let pixel_y = glyph.uv0[1] * atlas.height as f32;
        atlas
            .add_sprite("wide", 8, 512, &vec![255; 8 * 512 * 4])
            .unwrap();
        assert!(
            (atlas.font("numbers").glyph('8').unwrap().uv0[1] * atlas.height as f32 - pixel_y)
                .abs()
                < 0.01
        );
        let (a, b) = atlas.sprites["wide"];
        let mut batch = UiBatch::default();
        batch.image_region(
            &atlas,
            "wide",
            [0.0, 0.0, 50.0, 12.0],
            [0.0, 0.5, 0.0, 1.0],
            [1.0; 4],
        );
        assert!(
            batch
                .vertices
                .iter()
                .all(|v| v.uv[0] <= a[0] + (b[0] - a[0]) * 0.5)
        );
        batch.vertices.clear();
        batch.image_region(
            &atlas,
            "wide",
            [0.0, 0.0, 50.0, 12.0],
            [1.0, 0.0, 0.0, 1.0],
            [1.0; 4],
        );
        assert_eq!(batch.vertices[0].uv[0], b[0]);
    }
    #[test]
    fn cooldown_sector_area_matches_remaining_fraction_at_half_and_full() {
        let atlas = Atlas::new(12.0);
        for (fraction, area) in [(0.0, 0.0), (0.5, 648.0), (1.0, 1296.0)] {
            let mut batch = UiBatch::default();
            batch.cooldown(&atlas, [0.0, 0.0, 36.0], fraction);
            let measured: f32 = batch
                .vertices
                .chunks(3)
                .map(|v| {
                    let a = v[0].pos;
                    let b = v[1].pos;
                    let c = v[2].pos;
                    ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
                })
                .sum();
            assert!((measured - area).abs() < 0.01);
        }
    }
}
