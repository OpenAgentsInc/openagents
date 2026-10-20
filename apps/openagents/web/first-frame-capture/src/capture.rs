//! Runs the AgentGraph view headless and records its first frame.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use gpui::{
    AppContext as _, AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Background,
    BorderStyle, Bounds, DevicePixels, EdgeFadeParams, HeadlessAppContext, Hsla, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformTextSystem, PrimitiveBatch, ScaledPixels, Size, TileId,
    TransformationMatrix, px, size,
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
    /// For a grayscale glyph, the same glyph rasterized for subpixel
    /// rendering.
    subpixel: Option<Twin>,
}

/// A glyph's subpixel rasterization: where it sits against the grayscale
/// tile's origin, its size, and its pixels (blue, green, red, alpha).
struct Twin {
    offset: (i32, i32),
    size: Size<DevicePixels>,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct RecorderState {
    tiles: HashMap<AtlasKey, AtlasTile>,
    pixels: HashMap<(u32, u32), Recorded>,
    next: u32,
}

/// A sprite atlas that keeps every tile's pixels instead of uploading them.
/// Each tile gets a texture of its own, so a tile is named by its texture.
///
/// GPUI's headless window never asks for subpixel glyphs, but the browser's
/// WebGPU renderer does (with dual-source blending, as in Chrome), so the
/// recorder also rasterizes each grayscale glyph the way a subpixel window
/// would, through the same text system.
pub struct Recorder {
    state: Mutex<RecorderState>,
    text: Arc<dyn PlatformTextSystem>,
}

impl Recorder {
    pub fn new(text: Arc<dyn PlatformTextSystem>) -> Self {
        Self {
            state: Mutex::default(),
            text,
        }
    }

    fn twin(&self, key: &AtlasKey) -> Result<Option<Twin>> {
        let AtlasKey::Glyph(params) = key else {
            return Ok(None);
        };
        if params.is_emoji || params.subpixel_rendering {
            return Ok(None);
        }
        let gray = self.text.glyph_raster_bounds(params)?;
        let mut twin = params.clone();
        twin.subpixel_rendering = true;
        let bounds = self.text.glyph_raster_bounds(&twin)?;
        if bounds.size.width.0 == 0 || bounds.size.height.0 == 0 {
            return Ok(None);
        }
        let (size, bytes) = self.text.rasterize_glyph(&twin, bounds)?;
        Ok(Some(Twin {
            offset: (
                bounds.origin.x.0 - gray.origin.x.0,
                bounds.origin.y.0 - gray.origin.y.0,
            ),
            size,
            bytes,
        }))
    }
}

impl PlatformAtlas for Recorder {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        if let Some(tile) = self.state.lock().unwrap().tiles.get(key) {
            return Ok(Some(*tile));
        }
        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };
        let subpixel = self.twin(key)?;
        let mut state = self.state.lock().unwrap();
        state.next += 1;
        let id = state.next;
        let kind = key.texture_kind();
        let tile = AtlasTile {
            texture_id: AtlasTextureId { index: id, kind },
            tile_id: TileId(id),
            padding: 0,
            bounds: Bounds {
                origin: Default::default(),
                size,
            },
        };
        state.pixels.insert(
            (id, id),
            Recorded {
                kind,
                size,
                bytes: bytes.into_owned(),
                subpixel,
            },
        );
        state.tiles.insert(key.clone(), tile);
        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        self.state.lock().unwrap().tiles.remove(key);
    }
}

/// The window's renderer: converts each scene it's asked to draw.
struct Capture {
    atlas: Arc<Recorder>,
    subpixel: bool,
    frame: Rc<RefCell<Option<Result<ff::Scene>>>>,
}

impl PlatformHeadlessRenderer for Capture {
    fn render_scene_to_image(
        &mut self,
        scene: &gpui::Scene,
        size: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        *self.frame.borrow_mut() = Some(convert(scene, size, &self.atlas, self.subpixel));
        Ok(image::RgbaImage::new(1, 1))
    }

    fn render_scene(&mut self, _: &gpui::Scene, _: Size<DevicePixels>) -> Result<()> {
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

/// Lays out and paints the graph `json` in a window of `css` layout pixels
/// and returns its first frame, with subpixel glyphs when `subpixel`.
pub fn capture(json: &str, css: (f32, f32), subpixel: bool) -> Result<ff::Scene> {
    let spec = openagents_web_agent_graph::parse(json).map_err(|e| anyhow!(e))?;
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        openagents_web_agent_graph::theme::SANS,
    ));
    text.add_fonts(openagents_web_agent_graph::assets::font_faces())?;
    let frame = Rc::new(RefCell::new(None));
    let atlas = Arc::new(Recorder::new(text.clone()));
    let renderer = {
        let (frame, atlas) = (frame.clone(), atlas.clone());
        move || {
            Some(Box::new(Capture {
                atlas: atlas.clone(),
                subpixel,
                frame: frame.clone(),
            }) as Box<dyn PlatformHeadlessRenderer>)
        }
    };
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), renderer);
    let window = cx.open_window(size(px(css.0), px(css.1)), move |_, cx| {
        cx.new(|_| openagents_web_agent_graph::GraphView::new(spec, false, Vec::new()))
    })?;
    cx.run_until_parked();
    let any = window.into();
    cx.update_window(any, |_, window, cx| window.draw(cx).clear())?;
    cx.capture_screenshot(any)?;
    let scene = frame
        .borrow_mut()
        .take()
        .context("the window drew no frame")?;
    scene
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

/// A glyph tile in the scene's atlas: its size, where it sits against
/// GPUI's grayscale tile, and its pixels, row by row.
struct Glyph {
    w: u16,
    h: u16,
    offset: (i32, i32),
    pixels: Vec<u8>,
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
    device: Size<DevicePixels>,
    atlas: &Recorder,
    subpixel: bool,
) -> Result<ff::Scene> {
    ensure!(
        scene.backdrop_blurs.is_empty(),
        "backdrop blurs aren't supported"
    );
    let mut t = Tables::default();
    let mut out = ff::Scene {
        width: u32::try_from(device.width.0)?,
        height: u32::try_from(device.height.0)?,
        scale: 2.0,
        gamma_ratios: gpui::get_gamma_correction_ratios(GAMMA),
        contrast: CONTRAST,
        subpixel_contrast: SUBPIXEL_CONTRAST,
        ..ff::Scene::default()
    };
    // Glyph tiles by texture, in first-use order, with their pixels.
    let mut tile_at: HashMap<u32, u16> = HashMap::new();
    let mut glyphs: Vec<Glyph> = vec![];
    let mut corner_at: HashMap<[u32; 4], u32> = HashMap::new();
    let recorded = atlas.state.lock().unwrap();
    let channels = if subpixel { 3 } else { 1 };
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
                    let id = s.tile.texture_id.index;
                    let tile = match tile_at.get(&id) {
                        Some(&at) => at,
                        None => {
                            let r = recorded
                                .pixels
                                .get(&(id, s.tile.tile_id.0))
                                .context("a glyph's pixels weren't recorded")?;
                            ensure!(
                                r.kind == AtlasTextureKind::Monochrome,
                                "only coverage glyphs are supported"
                            );
                            let (w, h) = (r.size.width.0, r.size.height.0);
                            ensure!(
                                r.bytes.len() == (w * h) as usize,
                                "a glyph has the wrong number of pixels"
                            );
                            ensure!(
                                s.bounds.size.width.0 == w as f32
                                    && s.bounds.size.height.0 == h as f32,
                                "a glyph is drawn at another size than its tile"
                            );
                            let glyph = if subpixel {
                                let twin = r
                                    .subpixel
                                    .as_ref()
                                    .context("a glyph has no subpixel rasterization")?;
                                let (w, h) = (twin.size.width.0, twin.size.height.0);
                                ensure!(
                                    twin.bytes.len() == (w * h * 4) as usize,
                                    "a subpixel glyph has the wrong number of pixels"
                                );
                                // Blue, green, red, alpha to red, green, blue.
                                let pixels: Vec<u8> = twin
                                    .bytes
                                    .chunks_exact(4)
                                    .flat_map(|px| [px[2], px[1], px[0]])
                                    .collect();
                                Glyph {
                                    w: u16::try_from(w)?,
                                    h: u16::try_from(h)?,
                                    offset: twin.offset,
                                    pixels,
                                }
                            } else {
                                Glyph {
                                    w: u16::try_from(w)?,
                                    h: u16::try_from(h)?,
                                    offset: (0, 0),
                                    pixels: r.bytes.clone(),
                                }
                            };
                            let at = u16::try_from(glyphs.len()).context("too many glyphs")?;
                            glyphs.push(glyph);
                            tile_at.insert(id, at);
                            at
                        }
                    };
                    let offset = glyphs[tile as usize].offset;
                    out.sprites.push(ff::Sprite {
                        x: s.bounds.origin.x.0 + offset.0 as f32,
                        y: s.bounds.origin.y.0 + offset.1 as f32,
                        mask: t.mask(&s.content_mask.bounds)?,
                        color: t.color(s.color)?,
                        tile,
                    });
                }
                let range = start..out.sprites.len() as u32;
                out.commands.push(if subpixel {
                    ff::Command::SubpixelSprites(range)
                } else {
                    ff::Command::Sprites(range)
                });
            }
            PrimitiveBatch::SubpixelSprites { .. } => bail!("subpixel glyphs aren't supported"),
            PrimitiveBatch::PolychromeSprites { .. } => bail!("images aren't supported"),
            PrimitiveBatch::Underlines(_) => bail!("underlines aren't supported"),
            PrimitiveBatch::Surfaces(_) => bail!("surfaces aren't supported"),
        }
    }
    let sizes: Vec<(u16, u16)> = glyphs.iter().map(|g| (g.w, g.h)).collect();
    let (aw, ah, places) = pack(&sizes)?;
    let mut pixels = vec![0u8; aw as usize * ah as usize * channels];
    for (g, &(x, y)) in glyphs.iter().zip(&places) {
        let row_bytes = g.w as usize * channels;
        for row in 0..g.h as usize {
            let to = ((y as usize + row) * aw as usize + x as usize) * channels;
            pixels[to..to + row_bytes]
                .copy_from_slice(&g.pixels[row * row_bytes..(row + 1) * row_bytes]);
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

    /// The real graph, both classes: deterministic and drawable.
    #[test]
    fn foremans_first_frame_is_deterministic_and_round_trips() {
        let json = serde_json::to_string(&agent_graph::foreman::graph()).unwrap();
        for class in &crate::CLASSES {
            let a = capture(&json, class.size, true).unwrap();
            let b = capture(&json, class.size, true).unwrap();
            assert_eq!(a, b, "{} differs between runs", class.name);
            assert_eq!(
                (a.width, a.height),
                (
                    (class.size.0 * 2.0).round() as u32,
                    (class.size.1 * 2.0).round() as u32
                )
            );
            assert!(!a.quads.is_empty() && !a.sprites.is_empty() && !a.paths.is_empty());
            assert_eq!(a.atlas.channels, 3);
            // Grayscale glyphs are the same frame with one-channel tiles.
            let gray = capture(&json, class.size, false).unwrap();
            assert_eq!((gray.atlas.channels, gray.quads.len()), (1, a.quads.len()));
            let bytes = a.encode().unwrap();
            assert_eq!(ff::Scene::decode(&bytes).unwrap(), a);
        }
    }
}
