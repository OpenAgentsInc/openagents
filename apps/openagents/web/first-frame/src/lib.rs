//! One frame of a GPUI view as a flat list of primitives, in a small binary
//! format that a paint-only WebGL2 module can draw before the full app has
//! loaded.
//!
//! GPUI turns each frame into a `Scene`: quads (backgrounds, borders, and
//! rounded corners), shadows, path triangles, and glyph sprites cut from a
//! texture atlas, drawn in batches by draw order. [`Scene`] holds the same
//! primitives in that order, with colors already converted to the RGBA the
//! renderer's shaders compute, and the atlas reduced to the glyph pixels the
//! frame uses.
//!
//! [`Scene::encode`] writes it column by column, so a run of similar
//! primitives (a dot grid, a line of glyphs) compresses well; the page sends
//! it deflated. [`Scene::decode`] reads it back and refuses anything out of
//! range: a wrong header, a truncated or padded buffer, a count larger than
//! the bytes left, an index past its table, a non-finite number, a tile
//! outside the atlas, or a draw command past its primitives.
//!
//! The format has no dependencies, so the painter stays a few kilobytes.

use std::ops::Range;

/// The first four bytes of every encoded scene.
pub const MAGIC: [u8; 4] = *b"OAFF";
/// The format's version. A reader refuses any other.
pub const VERSION: u16 = 1;
/// The largest frame side, in device pixels.
pub const MAX_SIDE: u32 = 8192;
/// The largest number of entries in any one table.
pub const MAX_COUNT: u32 = 1 << 20;

/// A rectangle in device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The overlap of two rectangles, empty when they don't meet.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = (self.x + self.w).min(other.x + other.w);
        let b = (self.y + self.h).min(other.y + other.h);
        Rect::new(x, y, (r - x).max(0.0), (b - y).max(0.0))
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let r = (self.x + self.w).max(other.x + other.w);
        let b = (self.y + self.h).max(other.y + other.h);
        Rect::new(x, y, r - x, b - y)
    }
}

/// A color as the renderer's shaders see it: red, green, blue, alpha.
pub type Rgba = [f32; 4];

/// A filled rectangle with optional rounded corners and border.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    pub bounds: Rect,
    /// Index into [`Scene::masks`]: the clip this quad draws inside.
    pub mask: u16,
    /// Index into [`Scene::colors`].
    pub background: u16,
    /// Index into [`Scene::colors`].
    pub border_color: u16,
    /// Top left, top right, bottom right, bottom left.
    pub radii: [f32; 4],
    /// Top, right, bottom, left.
    pub borders: [f32; 4],
}

/// A drop shadow, or an inset shadow clipped to its element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    pub bounds: Rect,
    pub radii: [f32; 4],
    pub mask: u16,
    pub color: u16,
    pub blur: f32,
    pub inset: bool,
    pub element: Rect,
    pub element_radii: [f32; 4],
}

/// A path triangle corner: its position, and the curve coordinates the
/// rasterizer uses for antialiased curves.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathVertex {
    pub x: f32,
    pub y: f32,
    pub s: f32,
    pub t: f32,
}

/// A filled path: triangles, three entries of [`Scene::path_indices`] each,
/// naming corners in [`Scene::path_vertices`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub color: u16,
    /// The path's bounds inside its clip.
    pub clip: Rect,
    pub indices: Range<u32>,
}

/// A glyph, drawn from an atlas tile at the tile's size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sprite {
    pub x: f32,
    pub y: f32,
    pub mask: u16,
    pub color: u16,
    pub tile: u16,
}

/// A color image or color glyph (GPUI's polychrome sprite), drawn from a
/// tile of [`Scene::image_atlas`] stretched over `bounds`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Image {
    pub bounds: Rect,
    pub mask: u16,
    /// Index into [`Scene::image_tiles`].
    pub tile: u16,
    pub opacity: f32,
    pub grayscale: bool,
    /// Top left, top right, bottom right, bottom left.
    pub radii: [f32; 4],
}

/// A rectangle of the atlas, in its pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tile {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// The glyph pixels: coverage bytes, row by row. With one channel, one
/// byte a pixel; with three (subpixel glyphs), red, green, and blue
/// coverage for each pixel in turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Atlas {
    pub width: u16,
    pub height: u16,
    pub channels: u8,
    pub pixels: Vec<u8>,
}

impl Default for Atlas {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            channels: 1,
            pixels: vec![],
        }
    }
}

/// One draw, in order: a batch of one kind of primitive.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Quads(Range<u32>),
    Shadows(Range<u32>),
    /// Paths are drawn into an offscreen layer, then the layer is copied
    /// through `sprites` (rectangles in [`Scene::path_sprites`]).
    Paths {
        paths: Range<u32>,
        sprites: Range<u32>,
    },
    /// Coverage glyphs, from the atlas's first channel.
    Sprites(Range<u32>),
    /// Glyphs from a three-channel atlas, blended a color channel at a
    /// time as GPUI's subpixel pipeline does.
    SubpixelSprites(Range<u32>),
    /// Images from the image atlas.
    Images(Range<u32>),
}

/// One frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The frame's size in device pixels.
    pub width: u32,
    pub height: u32,
    /// Device pixels per layout pixel.
    pub scale: f32,
    /// The text renderer's gamma correction ratios, and its grayscale and
    /// subpixel contrast.
    pub gamma_ratios: [f32; 4],
    pub contrast: f32,
    pub subpixel_contrast: f32,
    pub colors: Vec<Rgba>,
    pub masks: Vec<Rect>,
    pub quads: Vec<Quad>,
    pub shadows: Vec<Shadow>,
    pub paths: Vec<Path>,
    pub path_vertices: Vec<PathVertex>,
    pub path_indices: Vec<u32>,
    pub path_sprites: Vec<Rect>,
    pub sprites: Vec<Sprite>,
    pub tiles: Vec<Tile>,
    pub atlas: Atlas,
    /// Color images: red, green, blue, and straight alpha for each pixel.
    pub images: Vec<Image>,
    pub image_tiles: Vec<Tile>,
    pub image_atlas: Atlas,
    pub commands: Vec<Command>,
}

/// Why a buffer isn't a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Error {}

/// GPUI's `hsla_to_rgba` from its shaders, in the same `f32` steps.
pub fn hsla_to_rgba(h: f32, s: f32, l: f32, a: f32) -> Rgba {
    let h = h * 6.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (mut r, mut g, mut b) = (m, m, m);
    if (0.0..1.0).contains(&h) {
        r += c;
        g += x;
    } else if (1.0..2.0).contains(&h) {
        r += x;
        g += c;
    } else if (2.0..3.0).contains(&h) {
        g += c;
        b += x;
    } else if (3.0..4.0).contains(&h) {
        g += x;
        b += c;
    } else if (4.0..5.0).contains(&h) {
        r += x;
        b += c;
    } else {
        r += c;
        b += x;
    }
    [r, g, b, a]
}

const KIND_QUADS: u8 = 1;
const KIND_SHADOWS: u8 = 2;
const KIND_PATHS: u8 = 3;
const KIND_SPRITES: u8 = 4;
const KIND_SUBPIXEL_SPRITES: u8 = 5;
const KIND_IMAGES: u8 = 6;

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn count(&mut self, n: usize) {
        self.u32(n as u32);
    }
    fn rects<T>(&mut self, items: &[T], rect: impl Fn(&T) -> Rect) {
        for item in items {
            self.f32(rect(item).x);
        }
        for item in items {
            self.f32(rect(item).y);
        }
        for item in items {
            self.f32(rect(item).w);
        }
        for item in items {
            self.f32(rect(item).h);
        }
    }
    fn f32s<T, const N: usize>(&mut self, items: &[T], get: impl Fn(&T) -> [f32; N]) {
        for at in 0..N {
            for item in items {
                self.f32(get(item)[at]);
            }
        }
    }
    fn u16s<T>(&mut self, items: &[T], get: impl Fn(&T) -> u16) {
        for item in items {
            self.u16(get(item));
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).ok_or(Error("truncated"))?;
        let slice = self.bytes.get(self.at..end).ok_or(Error("truncated"))?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        let b = self.take(4)?;
        let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(Error("a number is not finite"))
        }
    }
    /// A table's length, refused when its records can't fit in what's left.
    fn count(&mut self, record: usize) -> Result<usize, Error> {
        let n = self.u32()?;
        if n > MAX_COUNT {
            return Err(Error("a table is too long"));
        }
        let n = n as usize;
        if n.saturating_mul(record) > self.bytes.len() - self.at {
            return Err(Error("a table is longer than the buffer"));
        }
        Ok(n)
    }
    fn rects<T>(
        &mut self,
        items: &mut [T],
        set: impl Fn(&mut T) -> &mut Rect,
    ) -> Result<(), Error> {
        for item in items.iter_mut() {
            set(item).x = self.f32()?;
        }
        for item in items.iter_mut() {
            set(item).y = self.f32()?;
        }
        for item in items.iter_mut() {
            set(item).w = self.f32()?;
        }
        for item in items.iter_mut() {
            set(item).h = self.f32()?;
        }
        Ok(())
    }
    fn f32s<T, const N: usize>(
        &mut self,
        items: &mut [T],
        set: impl Fn(&mut T) -> &mut [f32; N],
    ) -> Result<(), Error> {
        for at in 0..N {
            for item in items.iter_mut() {
                set(item)[at] = self.f32()?;
            }
        }
        Ok(())
    }
    fn u16s<T>(&mut self, items: &mut [T], set: impl Fn(&mut T) -> &mut u16) -> Result<(), Error> {
        for item in items.iter_mut() {
            *set(item) = self.u16()?;
        }
        Ok(())
    }
}

fn range(r: &Range<u32>, len: usize) -> Result<(), Error> {
    if r.start <= r.end && r.end as usize <= len {
        Ok(())
    } else {
        Err(Error("a range is outside its table"))
    }
}

fn index(i: u16, len: usize) -> Result<(), Error> {
    if (i as usize) < len {
        Ok(())
    } else {
        Err(Error("an index is outside its table"))
    }
}

fn sized(r: &Rect) -> Result<(), Error> {
    if r.w >= 0.0 && r.h >= 0.0 && [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Error("a rectangle has a negative or non-finite size"))
    }
}

fn finite(values: &[f32]) -> Result<(), Error> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Error("a number is not finite"))
    }
}

impl Scene {
    /// Checks every size, index, and range, as [`Scene::decode`] does.
    pub fn validate(&self) -> Result<(), Error> {
        if self.width == 0 || self.height == 0 || self.width > MAX_SIDE || self.height > MAX_SIDE {
            return Err(Error("the frame size is out of range"));
        }
        if !(self.scale.is_finite() && self.scale > 0.0 && self.scale <= 8.0) {
            return Err(Error("the scale is out of range"));
        }
        finite(&self.gamma_ratios)?;
        finite(&[self.contrast, self.subpixel_contrast])?;
        let tables = [
            self.colors.len(),
            self.masks.len(),
            self.quads.len(),
            self.shadows.len(),
            self.paths.len(),
            self.path_vertices.len(),
            self.path_indices.len(),
            self.path_sprites.len(),
            self.sprites.len(),
            self.tiles.len(),
            self.commands.len(),
        ];
        if tables.iter().any(|&n| n > MAX_COUNT as usize) {
            return Err(Error("a table is too long"));
        }
        for c in &self.colors {
            finite(c)?;
        }
        for m in self.masks.iter().chain(&self.path_sprites) {
            sized(m)?;
        }
        let (colors, masks) = (self.colors.len(), self.masks.len());
        for q in &self.quads {
            sized(&q.bounds)?;
            index(q.mask, masks)?;
            index(q.background, colors)?;
            index(q.border_color, colors)?;
            finite(&q.radii)?;
            finite(&q.borders)?;
        }
        for s in &self.shadows {
            sized(&s.bounds)?;
            sized(&s.element)?;
            index(s.mask, masks)?;
            index(s.color, colors)?;
            finite(&s.radii)?;
            finite(&s.element_radii)?;
            if !(s.blur.is_finite() && s.blur >= 0.0) {
                return Err(Error("a shadow's blur is out of range"));
            }
        }
        for v in &self.path_vertices {
            finite(&[v.x, v.y, v.s, v.t])?;
        }
        if self
            .path_indices
            .iter()
            .any(|&i| i as usize >= self.path_vertices.len())
        {
            return Err(Error("an index is outside its table"));
        }
        for p in &self.paths {
            index(p.color, colors)?;
            sized(&p.clip)?;
            range(&p.indices, self.path_indices.len())?;
            if (p.indices.end - p.indices.start) % 3 != 0 {
                return Err(Error("a path is not made of whole triangles"));
            }
        }
        let atlas = &self.atlas;
        if atlas.channels != 1 && atlas.channels != 3 {
            return Err(Error("the atlas has neither one channel nor three"));
        }
        if atlas.pixels.len()
            != atlas.width as usize * atlas.height as usize * atlas.channels as usize
        {
            return Err(Error("the atlas has the wrong number of pixels"));
        }
        for t in &self.tiles {
            if t.x as u32 + t.w as u32 > atlas.width as u32
                || t.y as u32 + t.h as u32 > atlas.height as u32
            {
                return Err(Error("a tile is outside the atlas"));
            }
        }
        for s in &self.sprites {
            finite(&[s.x, s.y])?;
            index(s.mask, masks)?;
            index(s.color, colors)?;
            index(s.tile, self.tiles.len())?;
        }
        for c in &self.commands {
            match c {
                Command::Quads(r) => range(r, self.quads.len())?,
                Command::Shadows(r) => range(r, self.shadows.len())?,
                Command::Paths { paths, sprites } => {
                    range(paths, self.paths.len())?;
                    range(sprites, self.path_sprites.len())?;
                }
                // Coverage glyphs read the first channel of either atlas.
                Command::Sprites(r) => range(r, self.sprites.len())?,
                Command::SubpixelSprites(r) => {
                    range(r, self.sprites.len())?;
                    if !r.is_empty() && atlas.channels != 3 {
                        return Err(Error("glyphs don't match the atlas's channels"));
                    }
                }
                Command::Images(r) => range(r, self.images.len())?,
            }
        }
        let images = &self.image_atlas;
        if !self.images.is_empty() && images.channels != 4 {
            return Err(Error("the image atlas doesn't have four channels"));
        }
        if images.pixels.len()
            != images.width as usize * images.height as usize * images.channels as usize
        {
            return Err(Error("the image atlas has the wrong number of pixels"));
        }
        for t in &self.image_tiles {
            if t.x as u32 + t.w as u32 > images.width as u32
                || t.y as u32 + t.h as u32 > images.height as u32
            {
                return Err(Error("a tile is outside the atlas"));
            }
        }
        for image in &self.images {
            sized(&image.bounds)?;
            index(image.mask, masks)?;
            index(image.tile, self.image_tiles.len())?;
            finite(&image.radii)?;
            if !(image.opacity.is_finite() && (0.0..=1.0).contains(&image.opacity)) {
                return Err(Error("an image's opacity is out of range"));
            }
        }
        Ok(())
    }

    /// The scene as bytes. Refuses a scene [`Scene::validate`] refuses.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut w = Writer(Vec::with_capacity(4096));
        w.0.extend_from_slice(&MAGIC);
        w.u16(VERSION);
        w.u16(0);
        w.u32(self.width);
        w.u32(self.height);
        w.f32(self.scale);
        for v in self.gamma_ratios {
            w.f32(v);
        }
        w.f32(self.contrast);
        w.f32(self.subpixel_contrast);

        w.count(self.colors.len());
        w.f32s(&self.colors, |c| *c);

        w.count(self.masks.len());
        w.rects(&self.masks, |m| *m);

        w.count(self.quads.len());
        w.rects(&self.quads, |q| q.bounds);
        w.u16s(&self.quads, |q| q.mask);
        w.u16s(&self.quads, |q| q.background);
        w.u16s(&self.quads, |q| q.border_color);
        w.f32s(&self.quads, |q| q.radii);
        w.f32s(&self.quads, |q| q.borders);

        w.count(self.shadows.len());
        w.rects(&self.shadows, |s| s.bounds);
        w.f32s(&self.shadows, |s| s.radii);
        w.u16s(&self.shadows, |s| s.mask);
        w.u16s(&self.shadows, |s| s.color);
        w.f32s(&self.shadows, |s| [s.blur]);
        for s in &self.shadows {
            w.u8(s.inset as u8);
        }
        w.rects(&self.shadows, |s| s.element);
        w.f32s(&self.shadows, |s| s.element_radii);

        w.count(self.path_vertices.len());
        w.f32s(&self.path_vertices, |v| [v.x, v.y, v.s, v.t]);

        // Each index as the difference from the one before: triangles of a
        // stroke name neighboring corners, so the differences repeat.
        w.count(self.path_indices.len());
        let mut last = 0u32;
        for &i in &self.path_indices {
            w.u32(i.wrapping_sub(last));
            last = i;
        }

        w.count(self.paths.len());
        w.u16s(&self.paths, |p| p.color);
        w.rects(&self.paths, |p| p.clip);
        for p in &self.paths {
            w.u32(p.indices.start);
        }
        for p in &self.paths {
            w.u32(p.indices.end - p.indices.start);
        }

        w.count(self.path_sprites.len());
        w.rects(&self.path_sprites, |r| *r);

        w.count(self.tiles.len());
        w.u16s(&self.tiles, |t| t.x);
        w.u16s(&self.tiles, |t| t.y);
        w.u16s(&self.tiles, |t| t.w);
        w.u16s(&self.tiles, |t| t.h);

        w.count(self.sprites.len());
        w.f32s(&self.sprites, |s| [s.x, s.y]);
        w.u16s(&self.sprites, |s| s.mask);
        w.u16s(&self.sprites, |s| s.color);
        w.u16s(&self.sprites, |s| s.tile);

        w.u16(self.atlas.width);
        w.u16(self.atlas.height);
        w.u8(self.atlas.channels);
        w.0.extend_from_slice(&self.atlas.pixels);

        w.count(self.image_tiles.len());
        w.u16s(&self.image_tiles, |t| t.x);
        w.u16s(&self.image_tiles, |t| t.y);
        w.u16s(&self.image_tiles, |t| t.w);
        w.u16s(&self.image_tiles, |t| t.h);

        w.count(self.images.len());
        w.rects(&self.images, |i| i.bounds);
        w.u16s(&self.images, |i| i.mask);
        w.u16s(&self.images, |i| i.tile);
        w.f32s(&self.images, |i| [i.opacity]);
        for i in &self.images {
            w.u8(i.grayscale as u8);
        }
        w.f32s(&self.images, |i| i.radii);

        w.u16(self.image_atlas.width);
        w.u16(self.image_atlas.height);
        w.u8(self.image_atlas.channels);
        w.0.extend_from_slice(&self.image_atlas.pixels);

        w.count(self.commands.len());
        for c in &self.commands {
            let (kind, r, extra) = match c {
                Command::Quads(r) => (KIND_QUADS, r, None),
                Command::Shadows(r) => (KIND_SHADOWS, r, None),
                Command::Paths { paths, sprites } => (KIND_PATHS, paths, Some(sprites)),
                Command::Sprites(r) => (KIND_SPRITES, r, None),
                Command::SubpixelSprites(r) => (KIND_SUBPIXEL_SPRITES, r, None),
                Command::Images(r) => (KIND_IMAGES, r, None),
            };
            w.u8(kind);
            w.u32(r.start);
            w.u32(r.end);
            if let Some(s) = extra {
                w.u32(s.start);
                w.u32(s.end);
            }
        }
        Ok(w.0)
    }

    /// Reads a scene, refusing anything malformed or out of range.
    pub fn decode(bytes: &[u8]) -> Result<Scene, Error> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err(Error("not a first-frame scene"));
        }
        if r.u16()? != VERSION {
            return Err(Error("an unknown scene version"));
        }
        if r.u16()? != 0 {
            return Err(Error("unknown scene flags"));
        }
        let mut s = Scene {
            width: r.u32()?,
            height: r.u32()?,
            scale: r.f32()?,
            ..Scene::default()
        };
        for v in &mut s.gamma_ratios {
            *v = r.f32()?;
        }
        s.contrast = r.f32()?;
        s.subpixel_contrast = r.f32()?;

        let n = r.count(16)?;
        s.colors = vec![[0.0; 4]; n];
        r.f32s(&mut s.colors, |c| c)?;

        let n = r.count(16)?;
        s.masks = vec![Rect::default(); n];
        r.rects(&mut s.masks, |m| m)?;

        let n = r.count(54)?;
        s.quads = vec![Quad::default(); n];
        r.rects(&mut s.quads, |q| &mut q.bounds)?;
        r.u16s(&mut s.quads, |q| &mut q.mask)?;
        r.u16s(&mut s.quads, |q| &mut q.background)?;
        r.u16s(&mut s.quads, |q| &mut q.border_color)?;
        r.f32s(&mut s.quads, |q| &mut q.radii)?;
        r.f32s(&mut s.quads, |q| &mut q.borders)?;

        let n = r.count(85)?;
        s.shadows = vec![Shadow::default(); n];
        r.rects(&mut s.shadows, |s| &mut s.bounds)?;
        r.f32s(&mut s.shadows, |s| &mut s.radii)?;
        r.u16s(&mut s.shadows, |s| &mut s.mask)?;
        r.u16s(&mut s.shadows, |s| &mut s.color)?;
        for shadow in &mut s.shadows {
            shadow.blur = r.f32()?;
        }
        for shadow in &mut s.shadows {
            shadow.inset = match r.u8()? {
                0 => false,
                1 => true,
                _ => return Err(Error("a shadow's inset flag is not 0 or 1")),
            };
        }
        r.rects(&mut s.shadows, |s| &mut s.element)?;
        r.f32s(&mut s.shadows, |s| &mut s.element_radii)?;

        let n = r.count(16)?;
        let mut vertices = vec![[0.0f32; 4]; n];
        r.f32s(&mut vertices, |v| v)?;
        s.path_vertices = vertices
            .into_iter()
            .map(|[x, y, s, t]| PathVertex { x, y, s, t })
            .collect();

        let n = r.count(4)?;
        s.path_indices = Vec::with_capacity(n);
        let mut last = 0u32;
        for _ in 0..n {
            last = last.wrapping_add(r.u32()?);
            s.path_indices.push(last);
        }

        let n = r.count(26)?;
        s.paths = vec![Path::default(); n];
        r.u16s(&mut s.paths, |p| &mut p.color)?;
        r.rects(&mut s.paths, |p| &mut p.clip)?;
        for p in &mut s.paths {
            p.indices.start = r.u32()?;
        }
        for p in &mut s.paths {
            let len = r.u32()?;
            p.indices.end = p
                .indices
                .start
                .checked_add(len)
                .ok_or(Error("a range is outside its table"))?;
        }

        let n = r.count(16)?;
        s.path_sprites = vec![Rect::default(); n];
        r.rects(&mut s.path_sprites, |m| m)?;

        let n = r.count(8)?;
        s.tiles = vec![Tile::default(); n];
        r.u16s(&mut s.tiles, |t| &mut t.x)?;
        r.u16s(&mut s.tiles, |t| &mut t.y)?;
        r.u16s(&mut s.tiles, |t| &mut t.w)?;
        r.u16s(&mut s.tiles, |t| &mut t.h)?;

        let n = r.count(14)?;
        s.sprites = vec![Sprite::default(); n];
        for sprite in &mut s.sprites {
            sprite.x = r.f32()?;
        }
        for sprite in &mut s.sprites {
            sprite.y = r.f32()?;
        }
        r.u16s(&mut s.sprites, |s| &mut s.mask)?;
        r.u16s(&mut s.sprites, |s| &mut s.color)?;
        r.u16s(&mut s.sprites, |s| &mut s.tile)?;

        s.atlas.width = r.u16()?;
        s.atlas.height = r.u16()?;
        s.atlas.channels = r.u8()?;
        if s.atlas.channels != 1 && s.atlas.channels != 3 {
            return Err(Error("the atlas has neither one channel nor three"));
        }
        let len = s.atlas.width as usize * s.atlas.height as usize * s.atlas.channels as usize;
        s.atlas.pixels = r.take(len)?.to_vec();

        let n = r.count(8)?;
        s.image_tiles = vec![Tile::default(); n];
        r.u16s(&mut s.image_tiles, |t| &mut t.x)?;
        r.u16s(&mut s.image_tiles, |t| &mut t.y)?;
        r.u16s(&mut s.image_tiles, |t| &mut t.w)?;
        r.u16s(&mut s.image_tiles, |t| &mut t.h)?;

        let n = r.count(41)?;
        s.images = vec![Image::default(); n];
        r.rects(&mut s.images, |i| &mut i.bounds)?;
        r.u16s(&mut s.images, |i| &mut i.mask)?;
        r.u16s(&mut s.images, |i| &mut i.tile)?;
        for image in &mut s.images {
            image.opacity = r.f32()?;
        }
        for image in &mut s.images {
            image.grayscale = match r.u8()? {
                0 => false,
                1 => true,
                _ => return Err(Error("an image's grayscale flag is not 0 or 1")),
            };
        }
        r.f32s(&mut s.images, |i| &mut i.radii)?;

        s.image_atlas.width = r.u16()?;
        s.image_atlas.height = r.u16()?;
        s.image_atlas.channels = r.u8()?;
        if !matches!(s.image_atlas.channels, 1 | 4) {
            return Err(Error("the image atlas doesn't have four channels"));
        }
        let len = s.image_atlas.width as usize
            * s.image_atlas.height as usize
            * s.image_atlas.channels as usize;
        s.image_atlas.pixels = r.take(len)?.to_vec();

        let n = r.count(9)?;
        s.commands = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = r.u8()?;
            let a = r.u32()?..r.u32()?;
            s.commands.push(match kind {
                KIND_QUADS => Command::Quads(a),
                KIND_SHADOWS => Command::Shadows(a),
                KIND_PATHS => Command::Paths {
                    paths: a,
                    sprites: r.u32()?..r.u32()?,
                },
                KIND_SPRITES => Command::Sprites(a),
                KIND_SUBPIXEL_SPRITES => Command::SubpixelSprites(a),
                KIND_IMAGES => Command::Images(a),
                _ => return Err(Error("an unknown draw command")),
            });
        }
        if r.at != bytes.len() {
            return Err(Error("bytes after the scene"));
        }
        s.validate()?;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scene {
        Scene {
            width: 200,
            height: 100,
            scale: 2.0,
            gamma_ratios: [0.1, -0.2, 0.3, -0.4],
            contrast: 1.0,
            subpixel_contrast: 0.5,
            colors: vec![[0.03, 0.03, 0.03, 1.0], [1.0, 0.5, 0.25, 0.42]],
            masks: vec![Rect::new(0.0, 0.0, 200.0, 100.0)],
            quads: vec![
                Quad {
                    bounds: Rect::new(10.0, 10.0, 50.0, 20.0),
                    mask: 0,
                    background: 0,
                    border_color: 1,
                    radii: [4.0; 4],
                    borders: [1.0; 4],
                },
                Quad {
                    bounds: Rect::new(12.5, 40.25, 3.0, 3.0),
                    background: 1,
                    radii: [1.5; 4],
                    ..Quad::default()
                },
            ],
            shadows: vec![Shadow {
                bounds: Rect::new(5.0, 5.0, 30.0, 30.0),
                radii: [2.0; 4],
                mask: 0,
                color: 1,
                blur: 4.0,
                inset: true,
                element: Rect::new(4.0, 4.0, 32.0, 32.0),
                element_radii: [3.0; 4],
            }],
            paths: vec![Path {
                color: 1,
                clip: Rect::new(0.0, 0.0, 20.0, 20.0),
                indices: 0..3,
            }],
            path_vertices: vec![
                PathVertex {
                    x: 0.0,
                    y: 0.0,
                    s: 0.0,
                    t: 1.0,
                },
                PathVertex {
                    x: 20.0,
                    y: 0.0,
                    s: 0.0,
                    t: 1.0,
                },
                PathVertex {
                    x: 0.0,
                    y: 20.0,
                    s: 0.0,
                    t: 1.0,
                },
            ],
            path_indices: vec![0, 1, 2],
            path_sprites: vec![Rect::new(0.0, 0.0, 20.0, 20.0)],
            sprites: vec![Sprite {
                x: 30.0,
                y: 31.0,
                mask: 0,
                color: 1,
                tile: 0,
            }],
            tiles: vec![Tile {
                x: 1,
                y: 1,
                w: 2,
                h: 2,
            }],
            atlas: Atlas {
                width: 4,
                height: 4,
                channels: 1,
                pixels: (0..16).map(|v| v * 16).collect(),
            },
            images: vec![],
            image_tiles: vec![],
            image_atlas: Atlas::default(),
            commands: vec![
                Command::Shadows(0..1),
                Command::Quads(0..2),
                Command::Paths {
                    paths: 0..1,
                    sprites: 0..1,
                },
                Command::Sprites(0..1),
            ],
        }
    }

    #[test]
    fn a_scene_round_trips() {
        let scene = sample();
        let bytes = scene.encode().unwrap();
        assert_eq!(&bytes[..4], &MAGIC);
        assert_eq!(Scene::decode(&bytes).unwrap(), scene);
        // An empty frame round trips too.
        let empty = Scene {
            width: 1,
            height: 1,
            scale: 1.0,
            ..Scene::default()
        };
        assert_eq!(Scene::decode(&empty.encode().unwrap()).unwrap(), empty);
        // A color image, stretched over its bounds.
        let mut images = sample();
        images.images = vec![Image {
            bounds: Rect::new(2.0, 3.0, 8.0, 8.0),
            mask: 0,
            tile: 0,
            opacity: 0.5,
            grayscale: true,
            radii: [2.0; 4],
        }];
        images.image_tiles = vec![Tile {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        }];
        images.image_atlas = Atlas {
            width: 2,
            height: 2,
            channels: 4,
            pixels: (0..16).collect(),
        };
        images.commands.push(Command::Images(0..1));
        let bytes = images.encode().unwrap();
        assert_eq!(Scene::decode(&bytes).unwrap(), images);
        let mut wrong = images.clone();
        wrong.image_atlas.channels = 3;
        wrong.image_atlas.pixels.truncate(12);
        assert_eq!(
            wrong.validate(),
            Err(Error("the image atlas doesn't have four channels"))
        );
        let mut wrong = images.clone();
        wrong.images[0].opacity = 2.0;
        assert_eq!(
            wrong.validate(),
            Err(Error("an image's opacity is out of range"))
        );
        let mut wrong = images;
        wrong.images[0].tile = 1;
        assert_eq!(
            wrong.validate(),
            Err(Error("an index is outside its table"))
        );
        // Subpixel glyphs from a three-channel atlas.
        let mut subpixel = sample();
        subpixel.subpixel_contrast = 0.5;
        subpixel.atlas.channels = 3;
        subpixel.atlas.pixels = (0..48).collect();
        subpixel.commands[3] = Command::SubpixelSprites(0..1);
        let bytes = subpixel.encode().unwrap();
        assert_eq!(Scene::decode(&bytes).unwrap(), subpixel);
    }

    #[test]
    fn every_truncation_and_any_trailing_byte_is_refused() {
        let bytes = sample().encode().unwrap();
        for len in 0..bytes.len() {
            assert!(Scene::decode(&bytes[..len]).is_err(), "length {len}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(Scene::decode(&longer), Err(Error("bytes after the scene")));
    }

    #[test]
    fn a_wrong_header_is_refused() {
        let mut bytes = sample().encode().unwrap();
        bytes[0] = b'X';
        assert_eq!(Scene::decode(&bytes), Err(Error("not a first-frame scene")));
        let mut bytes = sample().encode().unwrap();
        bytes[4] = 9;
        assert_eq!(
            Scene::decode(&bytes),
            Err(Error("an unknown scene version"))
        );
    }

    #[test]
    fn a_count_larger_than_the_buffer_is_refused_before_allocating() {
        let mut bytes = sample().encode().unwrap();
        // The colors count follows the 44-byte header.
        bytes[44..48].copy_from_slice(&(MAX_COUNT - 1).to_le_bytes());
        assert_eq!(
            Scene::decode(&bytes),
            Err(Error("a table is longer than the buffer"))
        );
        bytes[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Scene::decode(&bytes), Err(Error("a table is too long")));
    }

    #[test]
    fn out_of_range_scenes_are_refused() {
        let cases: Vec<(fn(&mut Scene), &str)> = vec![
            (|s| s.width = 0, "the frame size is out of range"),
            (
                |s| s.height = MAX_SIDE + 1,
                "the frame size is out of range",
            ),
            (|s| s.scale = 0.0, "the scale is out of range"),
            (|s| s.quads[0].mask = 1, "an index is outside its table"),
            (
                |s| s.quads[1].background = 7,
                "an index is outside its table",
            ),
            (|s| s.sprites[0].tile = 1, "an index is outside its table"),
            (
                |s| s.quads[0].bounds.w = -1.0,
                "a rectangle has a negative or non-finite size",
            ),
            (|s| s.colors[0][2] = f32::NAN, "a number is not finite"),
            (|s| s.tiles[0].w = 4, "a tile is outside the atlas"),
            (
                |s| s.atlas.pixels.pop().map(drop).unwrap_or(()),
                "the atlas has the wrong number of pixels",
            ),
            (
                |s| s.paths[0].indices = 0..2,
                "a path is not made of whole triangles",
            ),
            (
                |s| s.paths[0].indices = 0..6,
                "a range is outside its table",
            ),
            (|s| s.path_indices[1] = 3, "an index is outside its table"),
            (
                |s| s.commands.push(Command::Quads(1..3)),
                "a range is outside its table",
            ),
            (
                |s| {
                    s.commands.push(Command::Paths {
                        paths: 0..1,
                        sprites: 0..2,
                    })
                },
                "a range is outside its table",
            ),
            (
                |s| s.shadows[0].blur = -1.0,
                "a shadow's blur is out of range",
            ),
            (
                |s| s.atlas.channels = 3,
                "the atlas has the wrong number of pixels",
            ),
            (
                |s| s.commands[3] = Command::SubpixelSprites(0..1),
                "glyphs don't match the atlas's channels",
            ),
        ];
        for (break_it, why) in cases {
            let mut scene = sample();
            break_it(&mut scene);
            assert_eq!(scene.validate(), Err(Error(why)));
            assert_eq!(scene.encode(), Err(Error(why)));
        }
    }

    #[test]
    fn malformed_bytes_inside_a_valid_frame_are_refused() {
        let scene = sample();
        let bytes = scene.encode().unwrap();
        // An unknown command kind: the last command is a sprite batch of
        // 9 bytes at the end.
        let mut bad = bytes.clone();
        let at = bad.len() - 9;
        bad[at] = 99;
        assert_eq!(Scene::decode(&bad), Err(Error("an unknown draw command")));
        // A non-finite number where the scale is.
        let mut bad = bytes.clone();
        bad[16..20].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert_eq!(Scene::decode(&bad), Err(Error("a number is not finite")));
        // A sprite pointing past the tiles, written by hand.
        let mut bad = bytes;
        // Commands (4 + 9 + 9 + 17 + 9 bytes), the empty image tables and
        // atlas (4 + 4 + 5), the atlas pixels and header, then the sprite's
        // tile index.
        let tile_at = bad.len() - 48 - 13 - 16 - 5 - 2;
        // The sprite's tile index sits just before the atlas header.
        let atlas_header = tile_at + 2;
        assert_eq!(&bad[atlas_header..atlas_header + 5], &[4, 0, 4, 0, 1]);
        // An atlas with two channels.
        let mut two = bad.clone();
        two[atlas_header + 4] = 2;
        assert_eq!(
            Scene::decode(&two),
            Err(Error("the atlas has neither one channel nor three"))
        );
        bad[tile_at..tile_at + 2].copy_from_slice(&5u16.to_le_bytes());
        assert_eq!(
            Scene::decode(&bad),
            Err(Error("an index is outside its table"))
        );
    }

    #[test]
    fn colors_match_the_shader() {
        // #66b5ff, the info accent, through Hsla and back.
        let [r, g, b, a] = hsla_to_rgba(0.5833333, 1.0, 0.7, 1.0);
        assert!((r - 0.4).abs() < 1e-5 && (g - 0.7).abs() < 1e-3 && (b - 1.0).abs() < 1e-5);
        assert_eq!(a, 1.0);
        assert_eq!(hsla_to_rgba(0.0, 0.0, 0.5, 0.5), [0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn rects_intersect_and_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(a.intersect(&Rect::new(20.0, 20.0, 1.0, 1.0)).w, 0.0);
    }
}
