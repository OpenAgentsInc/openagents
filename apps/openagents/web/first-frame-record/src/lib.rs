//! Records a GPUI frame as an `openagents-first-frame` scene: the
//! primitives in draw order, colors as GPUI's shaders compute them, and the
//! glyph pixels the frame uses.
//!
//! [`Recorder`] is a sprite atlas that keeps each tile's pixels as GPUI
//! hands them over, alone (a native capture with no GPU) or in front of a
//! real atlas (the browser, recording the app's own frame). [`convert`]
//! turns a frame's `Scene` into the scene format, refusing anything the
//! first-frame painter can't draw the same way.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use gpui::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Background, BorderStyle, Bounds,
    DevicePixels, EdgeFadeParams, Hsla, PlatformAtlas, PrimitiveBatch, ScaledPixels, Size, TileId,
    TransformationMatrix,
};
use openagents_first_frame as ff;

/// GPUI's text gamma, grayscale contrast, and subpixel contrast on the
/// web, where its `ZED_FONTS_*` variables are never set
/// (`RenderingParameters` in `gpui_wgpu`).
const GAMMA: f32 = 1.8;
const CONTRAST: f32 = 1.0;
const SUBPIXEL_CONTRAST: f32 = 0.5;

/// A tile's pixels as GPUI's atlas received them.
struct Recorded {
    kind: AtlasTextureKind,
    size: Size<DevicePixels>,
    bytes: Vec<u8>,
}

/// A tile's identity in the atlas it was drawn from.
type TileKey = (u32, u8, u32);

fn tile_key(tile: &AtlasTile) -> TileKey {
    (
        tile.texture_id.index,
        tile.texture_id.kind as u8,
        tile.tile_id.0,
    )
}

#[derive(Default)]
struct RecorderState {
    tiles: HashMap<AtlasKey, AtlasTile>,
    pixels: HashMap<TileKey, Recorded>,
    bytes: usize,
    next: u32,
}

/// At most this many bytes of tile pixels are kept; past it, new tiles go
/// unrecorded and a frame that uses one can't be converted.
const MAX_BYTES: usize = 48 << 20;

/// A sprite atlas that keeps every tile's pixels as GPUI hands them over:
/// alone, where each tile gets a texture of its own, or in front of a real
/// atlas that does the drawing.
#[derive(Default)]
pub struct Recorder {
    state: Mutex<RecorderState>,
    inner: Option<Arc<dyn PlatformAtlas>>,
}

impl Recorder {
    /// A recorder in front of `inner`, which keeps drawing as before.
    pub fn over(inner: Arc<dyn PlatformAtlas>) -> Self {
        Self {
            state: Mutex::default(),
            inner: Some(inner),
        }
    }

    fn keep(
        &self,
        tile: &AtlasTile,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: Vec<u8>,
    ) {
        let mut state = self.state.lock().unwrap();
        if state.bytes + bytes.len() > MAX_BYTES {
            return;
        }
        state.bytes += bytes.len();
        if let Some(old) = state
            .pixels
            .insert(tile_key(tile), Recorded { kind, size, bytes })
        {
            state.bytes -= old.bytes.len();
        }
    }
}

impl PlatformAtlas for Recorder {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let kind = key.texture_kind();
        if let Some(inner) = &self.inner {
            let mut built: Option<(Size<DevicePixels>, Vec<u8>)> = None;
            let tile = inner.get_or_insert_with(key, &mut || {
                let made = build()?;
                if let Some((size, bytes)) = &made {
                    built = Some((*size, bytes.to_vec()));
                }
                Ok(made)
            })?;
            if let (Some(tile), Some((size, bytes))) = (&tile, built) {
                self.keep(tile, kind, size, bytes);
            }
            return Ok(tile);
        }
        if let Some(tile) = self.state.lock().unwrap().tiles.get(key) {
            return Ok(Some(*tile));
        }
        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };
        let tile = {
            let mut state = self.state.lock().unwrap();
            state.next += 1;
            let id = state.next;
            let tile = AtlasTile {
                texture_id: AtlasTextureId { index: id, kind },
                tile_id: TileId(id),
                padding: 0,
                bounds: Bounds {
                    origin: Default::default(),
                    size,
                },
            };
            state.tiles.insert(key.clone(), tile);
            tile
        };
        self.keep(&tile, kind, size, bytes.into_owned());
        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        if let Some(inner) = &self.inner {
            inner.remove(key);
        }
        self.state.lock().unwrap().tiles.remove(key);
    }
}

/// How to capture: the window's size in layout pixels, its scale (the
/// page's device pixel ratio), and whether text is subpixel, as the
/// browser's WebGPU renderer draws it where dual-source blending exists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub size: (f32, f32),
    pub scale: f32,
    pub subpixel: bool,
}

impl Frame {
    /// The frame's size in device pixels, as a canvas of `size` at `scale`
    /// measures it.
    pub fn device(&self) -> (u32, u32) {
        (
            (self.size.0 * self.scale).round() as u32,
            (self.size.1 * self.scale).round() as u32,
        )
    }
}

fn rect(b: &Bounds<ScaledPixels>) -> ff::Rect {
    ff::Rect::new(b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0)
}

fn corners(c: &gpui::Corners<ScaledPixels>) -> [f32; 4] {
    [
        c.top_left.0,
        c.top_right.0,
        c.bottom_right.0,
        c.bottom_left.0,
    ]
}

fn no_fade(fade: &EdgeFadeParams) -> Result<()> {
    ensure!(
        *fade == EdgeFadeParams::default(),
        "edge fades and rounded clips aren't supported"
    );
    Ok(())
}

fn solid(background: &Background) -> Result<Hsla> {
    background
        .as_solid()
        .context("gradient and pattern backgrounds aren't supported")
}

/// Interns colors and clips, in first-use order.
#[derive(Default)]
struct Tables {
    colors: Vec<ff::Rgba>,
    color_at: HashMap<[u32; 4], u16>,
    masks: Vec<ff::Rect>,
    mask_at: HashMap<[u32; 4], u16>,
}

impl Tables {
    fn color(&mut self, c: Hsla) -> Result<u16> {
        let rgba = ff::hsla_to_rgba(c.h, c.s, c.l, c.a);
        let key = rgba.map(f32::to_bits);
        if let Some(&at) = self.color_at.get(&key) {
            return Ok(at);
        }
        let at = u16::try_from(self.colors.len()).context("too many colors")?;
        self.colors.push(rgba);
        self.color_at.insert(key, at);
        Ok(at)
    }

    fn mask(&mut self, b: &Bounds<ScaledPixels>) -> Result<u16> {
        let r = rect(b);
        let key = [r.x, r.y, r.w, r.h].map(f32::to_bits);
        if let Some(&at) = self.mask_at.get(&key) {
            return Ok(at);
        }
        let at = u16::try_from(self.masks.len()).context("too many clips")?;
        self.masks.push(r);
        self.mask_at.insert(key, at);
        Ok(at)
    }
}

/// A glyph tile in the scene's atlas: its size, its kind, and its pixels,
/// row by row (one coverage byte a pixel, or blue, green, red, alpha).
struct Glyph {
    w: u16,
    h: u16,
    subpixel: bool,
    pixels: Vec<u8>,
}

/// The scene's tile for a sprite's atlas tile, adding its pixels on first
/// use.
fn glyph(
    glyphs: &mut Vec<Glyph>,
    tile_at: &mut HashMap<TileKey, u16>,
    recorded: &RecorderState,
    tile: &AtlasTile,
    bounds: &Bounds<ScaledPixels>,
    subpixel: bool,
) -> Result<u16> {
    let id = tile_key(tile);
    let at = match tile_at.get(&id) {
        Some(&at) => at,
        None => {
            let r = recorded
                .pixels
                .get(&id)
                .context("a glyph's pixels weren't recorded")?;
            let want = if subpixel {
                AtlasTextureKind::Subpixel
            } else {
                AtlasTextureKind::Monochrome
            };
            ensure!(r.kind == want, "a glyph's tile is of another kind");
            let (w, h) = (r.size.width.0, r.size.height.0);
            let bytes = if subpixel { 4 } else { 1 };
            ensure!(
                r.bytes.len() == (w * h * bytes) as usize,
                "a glyph has the wrong number of pixels"
            );
            let at = u16::try_from(glyphs.len()).context("too many glyphs")?;
            glyphs.push(Glyph {
                w: u16::try_from(w)?,
                h: u16::try_from(h)?,
                subpixel,
                pixels: r.bytes.clone(),
            });
            tile_at.insert(id, at);
            at
        }
    };
    let g = &glyphs[at as usize];
    ensure!(
        bounds.size.width.0 == g.w as f32 && bounds.size.height.0 == g.h as f32,
        "a glyph is drawn at another size than its tile"
    );
    Ok(at)
}

/// Packs tiles into rows, tallest first, a pixel apart.
fn pack(tiles: &[(u16, u16)]) -> Result<(u16, u16, Vec<(u16, u16)>)> {
    const WIDTH: u32 = 1024;
    let mut order: Vec<usize> = (0..tiles.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(tiles[i].1), i));
    let mut at = vec![(0u16, 0u16); tiles.len()];
    let (mut x, mut y, mut row, mut width) = (0u32, 0u32, 0u32, 0u32);
    for i in order {
        let (w, h) = (tiles[i].0 as u32, tiles[i].1 as u32);
        ensure!(w <= WIDTH, "a glyph is wider than the atlas");
        if x + w > WIDTH {
            x = 0;
            y += row + 1;
            row = 0;
        }
        at[i] = (x as u16, y as u16);
        width = width.max(x + w);
        x += w + 1;
        row = row.max(h);
    }
    let height = y + row;
    ensure!(height <= u16::MAX as u32, "the atlas is too tall");
    Ok((width as u16, height as u16, at))
}

/// GPUI's frame as a first-frame scene: the batches in draw order, colors
/// as the shaders compute them, and only the glyph tiles the frame uses.
/// Anything the painter can't draw the same way is refused, so a page
/// never shows a frame that differs from the app's.
pub fn convert(
    scene: &gpui::Scene,
    device: (u32, u32),
    scale: f32,
    atlas: &Recorder,
) -> Result<ff::Scene> {
    ensure!(
        scene.backdrop_blurs.is_empty(),
        "backdrop blurs aren't supported"
    );
    let mut t = Tables::default();
    let mut out = ff::Scene {
        width: device.0,
        height: device.1,
        scale,
        gamma_ratios: gpui::get_gamma_correction_ratios(GAMMA),
        contrast: CONTRAST,
        subpixel_contrast: SUBPIXEL_CONTRAST,
        ..ff::Scene::default()
    };
    // Glyph tiles by texture, in first-use order, with their pixels.
    let mut tile_at: HashMap<TileKey, u16> = HashMap::new();
    let mut glyphs: Vec<Glyph> = vec![];
    // Color images and glyphs, packed into their own atlas.
    let mut pictures: Vec<Glyph> = vec![];
    let mut picture_at: HashMap<TileKey, u16> = HashMap::new();
    let mut corner_at: HashMap<[u32; 4], u32> = HashMap::new();
    let recorded = atlas.state.lock().unwrap();
    for batch in scene.batches() {
        match batch {
            PrimitiveBatch::Quads(range) => {
                let start = out.quads.len() as u32;
                for q in &scene.quads[range] {
                    no_fade(&q.fade)?;
                    ensure!(
                        q.border_style == BorderStyle::Solid,
                        "dashed borders aren't supported"
                    );
                    let w = &q.border_widths;
                    out.quads.push(ff::Quad {
                        bounds: rect(&q.bounds),
                        mask: t.mask(&q.content_mask.bounds)?,
                        background: t.color(solid(&q.background)?)?,
                        border_color: t.color(q.border_color)?,
                        radii: corners(&q.corner_radii),
                        borders: [w.top.0, w.right.0, w.bottom.0, w.left.0],
                    });
                }
                out.commands
                    .push(ff::Command::Quads(start..out.quads.len() as u32));
            }
            PrimitiveBatch::Shadows(range) => {
                let start = out.shadows.len() as u32;
                for s in &scene.shadows[range] {
                    out.shadows.push(ff::Shadow {
                        bounds: rect(&s.bounds),
                        radii: corners(&s.corner_radii),
                        mask: t.mask(&s.content_mask.bounds)?,
                        color: t.color(s.color)?,
                        blur: s.blur_radius.0,
                        inset: s.inset != 0,
                        element: rect(&s.element_bounds),
                        element_radii: corners(&s.element_corner_radii),
                    });
                }
                out.commands
                    .push(ff::Command::Shadows(start..out.shadows.len() as u32));
            }
            PrimitiveBatch::Paths(range) => {
                let paths = &scene.paths[range];
                let start = out.paths.len() as u32;
                for p in paths {
                    ensure!(p.vertices.len() % 3 == 0, "a path isn't whole triangles");
                    // GPUI lists every triangle's corners; a stroke's
                    // triangles share most of them, so keep each once.
                    let first = out.path_indices.len() as u32;
                    for v in &p.vertices {
                        let corner = ff::PathVertex {
                            x: v.xy_position.x.0,
                            y: v.xy_position.y.0,
                            s: v.st_position.x,
                            t: v.st_position.y,
                        };
                        let key = [corner.x, corner.y, corner.s, corner.t].map(f32::to_bits);
                        let at = *corner_at.entry(key).or_insert_with(|| {
                            out.path_vertices.push(corner);
                            out.path_vertices.len() as u32 - 1
                        });
                        out.path_indices.push(at);
                    }
                    out.paths.push(ff::Path {
                        color: t.color(solid(&p.color)?)?,
                        clip: rect(&p.clipped_bounds()),
                        indices: first..out.path_indices.len() as u32,
                    });
                }
                // As `draw_paths_from_intermediate`: one sprite a path when
                // they share a draw order, else their union.
                let sprites = out.path_sprites.len() as u32;
                if paths.last().map(|p| p.order) == paths.first().map(|p| p.order) {
                    for p in paths {
                        out.path_sprites.push(rect(&p.clipped_bounds()));
                    }
                } else if let Some(first) = paths.first() {
                    let mut b = first.clipped_bounds();
                    for p in &paths[1..] {
                        b = b.union(&p.clipped_bounds());
                    }
                    out.path_sprites.push(rect(&b));
                }
                out.commands.push(ff::Command::Paths {
                    paths: start..out.paths.len() as u32,
                    sprites: sprites..out.path_sprites.len() as u32,
                });
            }
            PrimitiveBatch::MonochromeSprites { range, .. } => {
                let start = out.sprites.len() as u32;
                for s in &scene.monochrome_sprites[range] {
                    no_fade(&s.fade)?;
                    ensure!(s.blur == 0.0, "blurred glyphs aren't supported");
                    ensure!(
                        s.transformation == TransformationMatrix::unit(),
                        "transformed glyphs aren't supported"
                    );
                    let tile = glyph(
                        &mut glyphs,
                        &mut tile_at,
                        &recorded,
                        &s.tile,
                        &s.bounds,
                        false,
                    )?;
                    out.sprites.push(ff::Sprite {
                        x: s.bounds.origin.x.0,
                        y: s.bounds.origin.y.0,
                        mask: t.mask(&s.content_mask.bounds)?,
                        color: t.color(s.color)?,
                        tile,
                    });
                }
                out.commands
                    .push(ff::Command::Sprites(start..out.sprites.len() as u32));
            }
            PrimitiveBatch::SubpixelSprites { range, .. } => {
                let start = out.sprites.len() as u32;
                for s in &scene.subpixel_sprites[range] {
                    no_fade(&s.fade)?;
                    ensure!(
                        s.transformation == TransformationMatrix::unit(),
                        "transformed glyphs aren't supported"
                    );
                    let tile = glyph(
                        &mut glyphs,
                        &mut tile_at,
                        &recorded,
                        &s.tile,
                        &s.bounds,
                        true,
                    )?;
                    out.sprites.push(ff::Sprite {
                        x: s.bounds.origin.x.0,
                        y: s.bounds.origin.y.0,
                        mask: t.mask(&s.content_mask.bounds)?,
                        color: t.color(s.color)?,
                        tile,
                    });
                }
                out.commands.push(ff::Command::SubpixelSprites(
                    start..out.sprites.len() as u32,
                ));
            }
            PrimitiveBatch::PolychromeSprites { range, .. } => {
                let start = out.images.len() as u32;
                for s in &scene.polychrome_sprites[range] {
                    no_fade(&s.fade)?;
                    ensure!(
                        s.alpha_mask.feather <= 0.0,
                        "masked images aren't supported"
                    );
                    let key = tile_key(&s.tile);
                    let tile = match picture_at.get(&key) {
                        Some(&at) => at,
                        None => {
                            let r = recorded
                                .pixels
                                .get(&key)
                                .context("an image's pixels weren't recorded")?;
                            ensure!(
                                r.kind == AtlasTextureKind::Polychrome,
                                "an image's tile is of another kind"
                            );
                            let (w, h) = (r.size.width.0, r.size.height.0);
                            ensure!(
                                r.bytes.len() == (w * h * 4) as usize,
                                "an image has the wrong number of pixels"
                            );
                            let at = u16::try_from(pictures.len()).context("too many images")?;
                            pictures.push(Glyph {
                                w: u16::try_from(w)?,
                                h: u16::try_from(h)?,
                                subpixel: true,
                                pixels: r.bytes.clone(),
                            });
                            picture_at.insert(key, at);
                            at
                        }
                    };
                    out.images.push(ff::Image {
                        bounds: rect(&s.bounds),
                        mask: t.mask(&s.content_mask.bounds)?,
                        tile,
                        opacity: s.opacity,
                        grayscale: s.grayscale == gpui::PaddedBool32::from(true),
                        radii: corners(&s.corner_radii),
                    });
                }
                out.commands
                    .push(ff::Command::Images(start..out.images.len() as u32));
            }
            PrimitiveBatch::Underlines(_) => bail!("underlines aren't supported"),
            PrimitiveBatch::Surfaces(_) => bail!("surfaces aren't supported"),
        }
    }
    // One channel unless some glyph is subpixel; then every tile has three,
    // a coverage glyph's the same in each.
    let channels = if glyphs.iter().any(|g| g.subpixel) {
        3
    } else {
        1
    };
    let sizes: Vec<(u16, u16)> = glyphs.iter().map(|g| (g.w, g.h)).collect();
    let (aw, ah, places) = pack(&sizes)?;
    let mut pixels = vec![0u8; aw as usize * ah as usize * channels];
    for (g, &(x, y)) in glyphs.iter().zip(&places) {
        for row in 0..g.h as usize {
            for col in 0..g.w as usize {
                let to = ((y as usize + row) * aw as usize + x as usize + col) * channels;
                let px = row * g.w as usize + col;
                if g.subpixel {
                    // Blue, green, red, alpha to red, green, blue.
                    let b = &g.pixels[px * 4..px * 4 + 4];
                    pixels[to..to + 3].copy_from_slice(&[b[2], b[1], b[0]]);
                } else {
                    pixels[to..to + channels].fill(g.pixels[px]);
                }
            }
        }
        out.tiles.push(ff::Tile {
            x,
            y,
            w: g.w,
            h: g.h,
        });
    }
    out.atlas = ff::Atlas {
        width: aw,
        height: ah,
        channels: channels as u8,
        pixels,
    };
    // Images: blue, green, red, alpha to red, green, blue, alpha.
    let sizes: Vec<(u16, u16)> = pictures.iter().map(|g| (g.w, g.h)).collect();
    let (iw, ih, places) = pack(&sizes)?;
    let mut pixels = vec![0u8; iw as usize * ih as usize * 4];
    for (g, &(x, y)) in pictures.iter().zip(&places) {
        for row in 0..g.h as usize {
            for col in 0..g.w as usize {
                let to = ((y as usize + row) * iw as usize + x as usize + col) * 4;
                let px = (row * g.w as usize + col) * 4;
                let b = &g.pixels[px..px + 4];
                pixels[to..to + 4].copy_from_slice(&[b[2], b[1], b[0], b[3]]);
            }
        }
        out.image_tiles.push(ff::Tile {
            x,
            y,
            w: g.w,
            h: g.h,
        });
    }
    out.image_atlas = ff::Atlas {
        width: iw,
        height: ih,
        channels: 4,
        pixels,
    };
    out.colors = t.colors;
    out.masks = t.masks;
    out.validate().map_err(|e| anyhow!(e))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_pack_without_overlap() {
        let tiles = [(10, 20), (1000, 5), (30, 20), (5, 5)];
        let (w, h, at) = pack(&tiles).unwrap();
        assert!(w <= 1024);
        for i in 0..tiles.len() {
            assert!(at[i].0 + tiles[i].0 <= w && at[i].1 + tiles[i].1 <= h);
            for j in 0..i {
                let a = (at[i], tiles[i]);
                let b = (at[j], tiles[j]);
                let apart = a.0.0 + a.1.0 <= b.0.0
                    || b.0.0 + b.1.0 <= a.0.0
                    || a.0.1 + a.1.1 <= b.0.1
                    || b.0.1 + b.1.1 <= a.0.1;
                assert!(apart, "{i} and {j} overlap");
            }
        }
        assert_eq!(pack(&[]).unwrap(), (0, 0, vec![]));
    }
}
