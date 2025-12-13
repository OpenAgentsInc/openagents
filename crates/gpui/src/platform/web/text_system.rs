//! Web text system using cosmic-text

use crate::{
    platform::PlatformTextSystem, Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun,
    GlyphId, LineLayout, Pixels, Point, RenderGlyphParams, ShapedGlyph, ShapedRun, Size,
};
use anyhow::Result;
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use parking_lot::RwLock;
use std::borrow::Cow;

/// Web text system using cosmic-text
pub struct WebTextSystem {
    font_system: RwLock<FontSystem>,
    swash_cache: RwLock<SwashCache>,
}

impl WebTextSystem {
    pub fn new() -> Self {
        Self {
            font_system: RwLock::new(FontSystem::new()),
            swash_cache: RwLock::new(SwashCache::new()),
        }
    }
}

impl Default for WebTextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformTextSystem for WebTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let mut font_system = self.font_system.write();
        for font_data in fonts {
            font_system.db_mut().load_font_data(font_data.into_owned());
        }
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        let font_system = self.font_system.read();
        font_system
            .db()
            .faces()
            .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .collect()
    }

    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        // For now, return a placeholder font ID
        // A full implementation would look up the font in the database
        Ok(FontId(0))
    }

    fn font_metrics(&self, _font_id: FontId) -> FontMetrics {
        // Return reasonable defaults for now
        FontMetrics {
            units_per_em: 1000,
            ascent: 800.0,
            descent: -200.0,
            line_gap: 0.0,
            underline_position: -100.0,
            underline_thickness: 50.0,
            cap_height: 700.0,
            x_height: 500.0,
            bounding_box: Bounds {
                origin: Point { x: 0.0, y: -200.0 },
                size: Size {
                    width: 1000.0,
                    height: 1000.0,
                },
            },
        }
    }

    fn typographic_bounds(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Ok(Bounds {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 600.0,
                height: 800.0,
            },
        })
    }

    fn advance(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(Size {
            width: 600.0,
            height: 0.0,
        })
    }

    fn glyph_for_char(&self, _font_id: FontId, ch: char) -> Option<GlyphId> {
        Some(GlyphId(ch as u32))
    }

    fn glyph_raster_bounds(&self, _params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Ok(Bounds::default())
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let width = raster_bounds.size.width.0 as usize;
        let height = raster_bounds.size.height.0 as usize;

        // TODO: Use swash to rasterize the glyph
        // For now, return empty data
        Ok((raster_bounds.size, vec![0u8; width * height]))
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        let mut font_system = self.font_system.write();

        let metrics = Metrics::new(font_size.0, font_size.0 * 1.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        buffer.set_text(
            &mut font_system,
            text,
            Attrs::new(),
            Shaping::Advanced,
        );

        let mut glyphs = Vec::new();
        let mut width = Pixels(0.0);

        // Process layout runs
        for line in buffer.lines.iter() {
            if let Some(layout) = line.layout_opt() {
                for layout_line in layout.iter() {
                    for glyph in layout_line.glyphs.iter() {
                        glyphs.push(ShapedGlyph {
                            id: GlyphId(glyph.glyph_id as u32),
                            position: Point {
                                x: Pixels(glyph.x),
                                y: Pixels(glyph.y),
                            },
                            index: glyph.start,
                            is_emoji: false,
                        });
                        width = Pixels(width.0.max(glyph.x + glyph.w));
                    }
                }
            }
        }

        let mut shaped_runs = Vec::new();
        if !glyphs.is_empty() {
            shaped_runs.push(ShapedRun {
                font_id: FontId(0),
                glyphs,
            });
        }

        LineLayout {
            font_size,
            width,
            ascent: Pixels(font_size.0 * 0.8),
            descent: Pixels(font_size.0 * -0.2),
            runs: shaped_runs,
            len: text.len(),
        }
    }
}
