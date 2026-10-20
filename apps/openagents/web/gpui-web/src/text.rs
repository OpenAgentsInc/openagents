//! A text system that lets the browser shape, measure, and draw glyphs
//! (`browser-text` feature, #12299).
//!
//! GPUI's default web text system is cosmic-text with swash, which ships a
//! shaper, a rasterizer, and a font database inside the module: about
//! 358 KB with brotli. The browser already has all three. This system:
//!
//! - registers the app's font files with the page (`FontFace`), reading
//!   each file's family, weight, and style from its `name` and `OS/2`
//!   tables;
//! - splits text into grapheme clusters, gives each cluster a glyph ID, and
//!   advances by the cluster's width from Canvas `measureText` (measured
//!   at 1000 px, so a font's "units per em" are 1000);
//! - draws each glyph with `fillText` into an `OffscreenCanvas` and hands
//!   GPUI its coverage, or its colors for emoji, for the atlas.
//!
//! Missing glyphs (CJK, emoji, symbols) fall back to the browser's own
//! fonts. What it gives up: kerning between clusters (each cluster advances
//! by its own width), and subpixel text (a canvas draws grayscale coverage),
//! so it recommends grayscale rendering.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow};
use gpui::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, FontStyle, GlyphId, LineLayout,
    Pixels, PlatformTextSystem, RenderGlyphParams, SUBPIXEL_VARIANTS_X, ShapedGlyph, ShapedRun,
    Size, TextRenderingMode, point, px, size,
};
use parking_lot::RwLock;
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::JsCast;

/// Font units per em: advances and metrics are measured at this size.
const EM: f32 = 1000.0;

/// Glyph IDs at and above this name clusters of more than one character;
/// below it, a glyph ID is the character itself.
const CLUSTERS: u32 = 0x11_0000;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Face {
    family: String,
    weight: u16,
    italic: bool,
}

#[derive(Default)]
struct State {
    faces: Vec<Face>,
    face_at: HashMap<Face, FontId>,
    families: Vec<String>,
    clusters: Vec<String>,
    cluster_at: HashMap<String, u32>,
    advances: HashMap<(FontId, GlyphId), f32>,
    metrics: HashMap<FontId, FontMetrics>,
}

/// Canvases are main-thread objects, so they live beside the shared state.
struct Canvases {
    measure: web_sys::OffscreenCanvasRenderingContext2d,
    raster: web_sys::OffscreenCanvas,
    paint: web_sys::OffscreenCanvasRenderingContext2d,
    loading: Vec<js_sys::Promise>,
}

thread_local! {
    static CANVASES: RefCell<Option<Canvases>> = const { RefCell::new(None) };
}

fn context(
    canvas: &web_sys::OffscreenCanvas,
) -> Result<web_sys::OffscreenCanvasRenderingContext2d> {
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"willReadFrequently".into(), &true.into()).ok();
    canvas
        .get_context_with_context_options("2d", &options)
        .map_err(|e| anyhow!("no 2d context: {e:?}"))?
        .context("no 2d context")?
        .dyn_into()
        .map_err(|_| anyhow!("not a 2d context"))
}

fn with_canvases<R>(f: impl FnOnce(&mut Canvases) -> Result<R>) -> Result<R> {
    CANVASES.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            let measure = web_sys::OffscreenCanvas::new(1, 1)
                .map_err(|e| anyhow!("no OffscreenCanvas: {e:?}"))?;
            let raster = web_sys::OffscreenCanvas::new(1, 1)
                .map_err(|e| anyhow!("no OffscreenCanvas: {e:?}"))?;
            *cell = Some(Canvases {
                measure: context(&measure)?,
                paint: context(&raster)?,
                raster,
                loading: Vec::new(),
            });
        }
        f(cell.as_mut().expect("just made"))
    })
}

/// The font files' load promises, to wait on before laying anything out.
pub(crate) fn fonts_loaded() -> js_sys::Promise {
    let all = js_sys::Array::new();
    CANVASES.with(|cell| {
        if let Some(canvases) = cell.borrow_mut().as_mut() {
            for promise in canvases.loading.drain(..) {
                all.push(&promise);
            }
        }
    });
    js_sys::Promise::all(&all)
}

/// Whether a cluster draws in color: pictographs and flags, and any cluster
/// asking for emoji presentation or joining emoji, unless it asks for text
/// presentation.
fn is_emoji(cluster: &str) -> bool {
    if cluster.contains('\u{FE0E}') {
        return false;
    }
    if cluster.contains('\u{FE0F}') || cluster.contains('\u{200D}') {
        return true;
    }
    cluster
        .chars()
        .next()
        .is_some_and(|c| matches!(c as u32, 0x1F000..=0x1FAFF))
}

/// A font file's family, weight, and style, from its `name` and `OS/2`
/// tables.
pub(crate) fn describe(bytes: &[u8]) -> Option<(String, u16, bool)> {
    let u16_at = |at: usize| -> Option<u16> {
        Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
    };
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let tables = u16_at(4)? as usize;
    let mut name = None;
    let mut os2 = None;
    for t in 0..tables {
        let record = 12 + t * 16;
        let tag = bytes.get(record..record + 4)?;
        let offset = u32_at(record + 8)? as usize;
        match tag {
            b"name" => name = Some(offset),
            b"OS/2" => os2 = Some(offset),
            _ => {}
        }
    }
    let (weight, italic) = match os2 {
        Some(at) => (u16_at(at + 4)?, u16_at(at + 62)? & 1 == 1),
        None => (400, false),
    };
    let name = name?;
    let count = u16_at(name + 2)? as usize;
    let strings = name + u16_at(name + 4)? as usize;
    let mut found: Option<(u16, String)> = None;
    for r in 0..count {
        let record = name + 6 + r * 12;
        let platform = u16_at(record)?;
        let id = u16_at(record + 6)?;
        let length = u16_at(record + 8)? as usize;
        let offset = u16_at(record + 10)? as usize;
        if (id != 1 && id != 16) || platform != 3 {
            continue;
        }
        let raw = bytes.get(strings + offset..strings + offset + length)?;
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect();
        let text = String::from_utf16(&units).ok()?;
        // The typographic family (16) over the legacy one (1).
        if found.as_ref().is_none_or(|(have, _)| id > *have) {
            found = Some((id, text));
        }
    }
    Some((found?.1, weight, italic))
}

/// The browser's text system for GPUI.
pub struct BrowserTextSystem {
    state: RwLock<State>,
    fallback: String,
}

impl BrowserTextSystem {
    /// `fallback` names the family for fonts GPUI asks for by a name the
    /// page doesn't have.
    pub fn new(fallback: &str) -> Self {
        Self {
            state: RwLock::new(State::default()),
            fallback: fallback.to_owned(),
        }
    }

    fn face(&self, id: FontId) -> Face {
        self.state.read().faces.get(id.0).cloned().unwrap_or(Face {
            family: self.fallback.clone(),
            weight: 400,
            italic: false,
        })
    }

    /// A CSS font for `face` at `pixels`, falling back to the browser's own
    /// fonts for what it lacks.
    fn css(&self, face: &Face, pixels: f32) -> String {
        let mono = face.family.to_ascii_lowercase().contains("mono");
        let generic = if mono {
            "ui-monospace, monospace"
        } else {
            "system-ui, sans-serif"
        };
        format!(
            "{}{} {}px \"{}\", {generic}",
            if face.italic { "italic " } else { "" },
            face.weight,
            pixels,
            face.family.replace('"', "")
        )
    }

    fn cluster(&self, glyph: GlyphId) -> String {
        if glyph.0 < CLUSTERS {
            char::from_u32(glyph.0)
                .map(String::from)
                .unwrap_or_default()
        } else {
            self.state
                .read()
                .clusters
                .get((glyph.0 - CLUSTERS) as usize)
                .cloned()
                .unwrap_or_default()
        }
    }

    fn glyph(&self, cluster: &str) -> GlyphId {
        let mut chars = cluster.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return GlyphId(c as u32);
        }
        let mut state = self.state.write();
        if let Some(&id) = state.cluster_at.get(cluster) {
            return GlyphId(id);
        }
        let id = CLUSTERS + state.clusters.len() as u32;
        state.clusters.push(cluster.to_owned());
        state.cluster_at.insert(cluster.to_owned(), id);
        GlyphId(id)
    }

    /// A cluster's advance in font units.
    fn measure(&self, font: FontId, glyph: GlyphId) -> f32 {
        if let Some(&width) = self.state.read().advances.get(&(font, glyph)) {
            return width;
        }
        let css = self.css(&self.face(font), EM);
        let text = self.cluster(glyph);
        let width = with_canvases(|c| {
            c.measure.set_font(&css);
            Ok(c.measure
                .measure_text(&text)
                .map_err(|e| anyhow!("{e:?}"))?
                .width() as f32)
        })
        .unwrap_or(0.0);
        self.state.write().advances.insert((font, glyph), width);
        width
    }
}

impl PlatformTextSystem for BrowserTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        for bytes in fonts {
            let Some((family, weight, italic)) = describe(&bytes) else {
                log::warn!("a font file has no readable family name; skipping it");
                continue;
            };
            let descriptors = web_sys::FontFaceDescriptors::new();
            descriptors.set_weight(&weight.to_string());
            descriptors.set_style(if italic { "italic" } else { "normal" });
            let buffer = js_sys::Uint8Array::from(bytes.as_ref()).buffer();
            let face = web_sys::FontFace::new_with_array_buffer_and_descriptors(
                &family,
                &buffer,
                &descriptors,
            )
            .map_err(|e| anyhow!("the browser refused a font: {e:?}"))?;
            let document = web_sys::window()
                .and_then(|w| w.document())
                .context("no document")?;
            document.fonts().add(&face).ok();
            let loading = face.load().map_err(|e| anyhow!("{e:?}"))?;
            with_canvases(|c| {
                c.loading.push(loading);
                Ok(())
            })?;
            let mut state = self.state.write();
            if !state.families.contains(&family) {
                state.families.push(family);
            }
        }
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        self.state.read().families.clone()
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        let face = Face {
            family: font.family.to_string(),
            weight: (font.weight.0.round() as u16).clamp(1, 1000),
            italic: !matches!(font.style, FontStyle::Normal),
        };
        let mut state = self.state.write();
        if let Some(&id) = state.face_at.get(&face) {
            return Ok(id);
        }
        let id = FontId(state.faces.len());
        state.faces.push(face.clone());
        state.face_at.insert(face, id);
        Ok(id)
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        if let Some(metrics) = self.state.read().metrics.get(&font_id) {
            return *metrics;
        }
        let css = self.css(&self.face(font_id), EM);
        let measured = with_canvases(|c| {
            c.measure.set_font(&css);
            let m = |text: &str| c.measure.measure_text(text).map_err(|e| anyhow!("{e:?}"));
            let line = m("Hg")?;
            let cap = m("H")?.actual_bounding_box_ascent() as f32;
            let x = m("x")?.actual_bounding_box_ascent() as f32;
            let ascent = line.font_bounding_box_ascent() as f32;
            let descent = line.font_bounding_box_descent() as f32;
            let width = m("M")?.width() as f32;
            Ok(FontMetrics {
                units_per_em: EM as u32,
                ascent,
                descent: -descent,
                line_gap: 0.0,
                underline_position: -0.1 * EM,
                underline_thickness: 0.05 * EM,
                cap_height: cap,
                x_height: x,
                bounding_box: Bounds {
                    origin: point(0.0, -descent),
                    size: size(width, ascent + descent),
                },
            })
        })
        .unwrap_or(FontMetrics {
            units_per_em: EM as u32,
            ascent: 0.9 * EM,
            descent: -0.25 * EM,
            line_gap: 0.0,
            underline_position: -0.1 * EM,
            underline_thickness: 0.05 * EM,
            cap_height: 0.7 * EM,
            x_height: 0.5 * EM,
            bounding_box: Bounds {
                origin: point(0.0, -0.25 * EM),
                size: size(EM, 1.15 * EM),
            },
        });
        self.state.write().metrics.insert(font_id, measured);
        measured
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        let metrics = self.font_metrics(font_id);
        Ok(Bounds {
            origin: point(0.0, 0.0),
            size: size(
                self.measure(font_id, glyph_id),
                metrics.ascent - metrics.descent,
            ),
        })
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(size(self.measure(font_id, glyph_id), 0.0))
    }

    fn glyph_for_char(&self, _font_id: FontId, ch: char) -> Option<GlyphId> {
        // The browser falls back for any character the font lacks.
        (!ch.is_control()).then_some(GlyphId(ch as u32))
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let text = self.cluster(params.glyph_id);
        if text.trim().is_empty() {
            return Ok(Bounds::default());
        }
        let pixels = f32::from(params.font_size) * params.scale_factor;
        let css = self.css(&self.face(params.font_id), pixels);
        let shift = params.subpixel_variant.x as f64 / SUBPIXEL_VARIANTS_X as f64;
        with_canvases(|c| {
            c.measure.set_font(&css);
            let m = c
                .measure
                .measure_text(&text)
                .map_err(|e| anyhow!("{e:?}"))?;
            let left = (shift - m.actual_bounding_box_left()).floor() as i32 - 1;
            let right = (shift + m.actual_bounding_box_right()).ceil() as i32 + 1;
            let top = (-m.actual_bounding_box_ascent()).floor() as i32 - 1;
            let bottom = m.actual_bounding_box_descent().ceil() as i32 + 1;
            if right <= left || bottom <= top {
                return Ok(Bounds::default());
            }
            Ok(Bounds {
                origin: point(DevicePixels(left), DevicePixels(top)),
                size: size(DevicePixels(right - left), DevicePixels(bottom - top)),
            })
        })
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let (w, h) = (raster_bounds.size.width.0, raster_bounds.size.height.0);
        anyhow::ensure!(w > 0 && h > 0, "glyph bounds are empty");
        let text = self.cluster(params.glyph_id);
        let pixels = f32::from(params.font_size) * params.scale_factor;
        let css = self.css(&self.face(params.font_id), pixels);
        let shift = params.subpixel_variant.x as f64 / SUBPIXEL_VARIANTS_X as f64;
        let rgba = with_canvases(|c| {
            if c.raster.width() < w as u32 || c.raster.height() < h as u32 {
                c.raster.set_width(c.raster.width().max(w as u32));
                c.raster.set_height(c.raster.height().max(h as u32));
            }
            c.paint.clear_rect(0.0, 0.0, w as f64, h as f64);
            c.paint.set_font(&css);
            c.paint.set_fill_style_str("#fff");
            c.paint
                .fill_text(
                    &text,
                    shift - raster_bounds.origin.x.0 as f64,
                    -raster_bounds.origin.y.0 as f64,
                )
                .map_err(|e| anyhow!("{e:?}"))?;
            let data = c
                .paint
                .get_image_data(0.0, 0.0, w as f64, h as f64)
                .map_err(|e| anyhow!("{e:?}"))?;
            Ok(data.data().0)
        })?;
        let bytes = if params.is_emoji {
            // Red, green, blue, alpha to the atlas's blue, green, red, alpha.
            let mut bgra = rgba;
            for px in bgra.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            bgra
        } else {
            let alpha = rgba.chunks_exact(4).map(|px| px[3]);
            if params.subpixel_rendering {
                alpha.flat_map(|a| [a, a, a, a]).collect()
            } else {
                alpha.collect()
            }
        };
        Ok((raster_bounds.size, bytes))
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        let scale = f32::from(font_size) / EM;
        let mut shaped: Vec<ShapedRun> = Vec::new();
        let (mut x, mut ascent, mut descent) = (0.0f32, 0.0f32, 0.0f32);
        let mut start = 0;
        for run in runs {
            let end = (start + run.len).min(text.len());
            let metrics = self.font_metrics(run.font_id);
            ascent = ascent.max(metrics.ascent * scale);
            descent = descent.max(-metrics.descent * scale);
            let mut glyphs = Vec::new();
            for (at, cluster) in text[start..end].grapheme_indices(true) {
                let id = self.glyph(cluster);
                glyphs.push(ShapedGlyph {
                    id,
                    position: point(px(x), px(0.0)),
                    index: start + at,
                    is_emoji: is_emoji(cluster),
                });
                x += self.measure(run.font_id, id) * scale;
            }
            if !glyphs.is_empty() {
                shaped.push(ShapedRun {
                    font_id: run.font_id,
                    glyphs,
                });
            }
            start = end;
        }
        LineLayout {
            font_size,
            width: px(x),
            ascent: px(ascent),
            descent: px(descent),
            runs: shaped,
            len: text.len(),
        }
    }

    fn recommended_rendering_mode(
        &self,
        _font_id: FontId,
        _font_size: Pixels,
    ) -> TextRenderingMode {
        // A canvas draws grayscale coverage only.
        TextRenderingMode::Grayscale
    }
}
