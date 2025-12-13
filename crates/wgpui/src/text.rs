//! Text system using cosmic-text for shaping and a glyph atlas for GPU rendering.

use crate::color::Hsla;
use crate::geometry::{Point, Size};
use crate::scene::{GlyphInstance, TextRun};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent,
    Weight,
};

/// Default font family for text rendering
#[cfg(target_arch = "wasm32")]
const DEFAULT_FONT_FAMILY: Family<'static> = Family::Name("Berkeley Mono");
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_FONT_FAMILY: Family<'static> = Family::Monospace;

/// Default shaping mode - use Basic for WASM (simpler, no HarfBuzz)
#[cfg(target_arch = "wasm32")]
const DEFAULT_SHAPING: Shaping = Shaping::Basic;
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_SHAPING: Shaping = Shaping::Advanced;
use std::collections::HashMap;

/// Font style for text rendering.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
}

/// Default atlas size (1024x1024 single-channel texture)
const ATLAS_SIZE: u32 = 1024;

/// Cached glyph entry in the atlas.
#[derive(Clone, Debug)]
struct GlyphEntry {
    /// UV coordinates (normalized 0-1)
    uv: [f32; 4], // min_u, min_v, max_u, max_v
    /// Glyph size in pixels
    size: Size,
    /// Bearing offset
    offset: Point,
}

/// Cache key for glyphs.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct GlyphCacheKey {
    cache_key: CacheKey,
}

/// Text system managing fonts, shaping, and glyph atlas.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    glyph_cache: HashMap<GlyphCacheKey, GlyphEntry>,
    atlas_data: Vec<u8>,
    atlas_size: u32,
    atlas_cursor_x: u32,
    atlas_cursor_y: u32,
    atlas_row_height: u32,
    dirty: bool,
    scale_factor: f32,
}

impl TextSystem {
    pub fn new(scale_factor: f32) -> Self {
        // For WASM, create font system with embedded fonts (all variants)
        #[cfg(target_arch = "wasm32")]
        let font_system = {
            use cosmic_text::fontdb;
            let mut db = fontdb::Database::new();

            // Load all font variants
            let regular = include_bytes!("../fonts/BerkeleyMono-Regular.ttf");
            let bold = include_bytes!("../fonts/BerkeleyMono-Bold.ttf");
            let italic = include_bytes!("../fonts/BerkeleyMono-Italic.ttf");
            let bold_italic = include_bytes!("../fonts/BerkeleyMono-BoldItalic.ttf");

            db.load_font_data(regular.to_vec());
            db.load_font_data(bold.to_vec());
            db.load_font_data(italic.to_vec());
            db.load_font_data(bold_italic.to_vec());

            FontSystem::new_with_locale_and_db("en-US".to_string(), db)
        };

        // For native, use system fonts
        #[cfg(not(target_arch = "wasm32"))]
        let font_system = FontSystem::new();

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            glyph_cache: HashMap::new(),
            atlas_data: vec![0u8; (ATLAS_SIZE * ATLAS_SIZE) as usize],
            atlas_size: ATLAS_SIZE,
            atlas_cursor_x: 0,
            atlas_cursor_y: 0,
            atlas_row_height: 0,
            dirty: true,
            scale_factor,
        }
    }

    /// Update scale factor (invalidates glyph cache).
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        if (self.scale_factor - scale_factor).abs() > 0.001 {
            self.scale_factor = scale_factor;
            self.clear_cache();
        }
    }

    /// Clear glyph cache (e.g., on scale factor change).
    pub fn clear_cache(&mut self) {
        self.glyph_cache.clear();
        self.atlas_data.fill(0);
        self.atlas_cursor_x = 0;
        self.atlas_cursor_y = 0;
        self.atlas_row_height = 0;
        self.dirty = true;
    }

    /// Check if atlas needs to be uploaded to GPU.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark atlas as clean after GPU upload.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Get atlas data for GPU upload.
    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    /// Get atlas size.
    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }

    /// Measure text width without rendering.
    pub fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        // Simple calculation: character count * advance width
        let char_count = text.chars().count() as f32;
        char_count * font_size * 1.1
    }

    /// Measure text and return size.
    pub fn measure_size(&mut self, text: &str, font_size: f32, _max_width: Option<f32>) -> Size {
        // Simple calculation for monospace font
        let char_count = text.chars().count() as f32;
        let width = char_count * font_size * 1.1;
        let height = font_size * 1.6;
        Size::new(width, height)
    }

    /// Layout text and return a TextRun for rendering.
    pub fn layout(&mut self, text: &str, origin: Point, font_size: f32, color: Hsla) -> TextRun {
        self.layout_styled(text, origin, font_size, color, FontStyle::default())
    }

    /// Layout text with font style (bold/italic) and return a TextRun for rendering.
    pub fn layout_styled(
        &mut self,
        text: &str,
        origin: Point,
        font_size: f32,
        color: Hsla,
        style: FontStyle,
    ) -> TextRun {
        let physical_font_size = font_size * self.scale_factor;
        let metrics = Metrics::new(physical_font_size, physical_font_size * 1.2);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(10000.0), None);

        // Build attrs with correct weight and style
        let attrs = Attrs::new()
            .family(DEFAULT_FONT_FAMILY)
            .weight(if style.bold { Weight::BOLD } else { Weight::NORMAL })
            .style(if style.italic { Style::Italic } else { Style::Normal });

        buffer.set_text(&mut self.font_system, text, attrs, DEFAULT_SHAPING);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut text_run = TextRun::new(origin, color, font_size);

        // Collect glyph data first to avoid borrow checker issues
        let mut glyph_data: Vec<(GlyphCacheKey, f32, f32, u16)> = Vec::new();

        for run in buffer.layout_runs() {
            // Manually track x position by accumulating glyph widths
            let mut current_x = 0.0f32;
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0.0, 0.0), self.scale_factor);
                let cache_key = GlyphCacheKey {
                    cache_key: physical_glyph.cache_key,
                };
                glyph_data.push((cache_key, current_x, run.line_y, glyph.glyph_id as u16));
                // Monospace font advance width
                let advance = physical_font_size * 1.1;
                current_x += advance;
            }
        }

        // Now process glyphs, rasterizing as needed
        for (cache_key, glyph_x, line_y, glyph_id) in glyph_data {
            // Get or rasterize glyph
            let entry = if let Some(entry) = self.glyph_cache.get(&cache_key) {
                entry.clone()
            } else {
                // Rasterize glyph - clone data immediately to release borrow
                let image_data: Option<(i32, i32, u32, u32, SwashContent, Vec<u8>)> =
                    self.swash_cache.get_image(&mut self.font_system, cache_key.cache_key)
                        .as_ref()
                        .map(|image| {
                            (
                                image.placement.left,
                                image.placement.top,
                                image.placement.width,
                                image.placement.height,
                                image.content,
                                image.data.to_vec(),
                            )
                        });

                if let Some((left, top, width, height, content, data)) = image_data {
                    let placement = cosmic_text::Placement { left, top, width, height };
                    let entry = self.pack_glyph_data(&placement, content, &data);
                    self.glyph_cache.insert(cache_key, entry.clone());
                    entry
                } else {
                    // Skip if rasterization failed
                    continue;
                }
            };

            // Create glyph instance
            let glyph_instance = GlyphInstance {
                glyph_id,
                offset: Point::new(
                    (glyph_x + entry.offset.x) / self.scale_factor,
                    (line_y + entry.offset.y) / self.scale_factor,
                ),
                size: Size::new(
                    entry.size.width / self.scale_factor,
                    entry.size.height / self.scale_factor,
                ),
                uv: entry.uv,
            };

            text_run.push_glyph(glyph_instance);
        }

        text_run
    }

    /// Pack a glyph into the atlas.
    fn pack_glyph_data(
        &mut self,
        placement: &cosmic_text::Placement,
        content: SwashContent,
        glyph_data: &[u8],
    ) -> GlyphEntry {
        let width = placement.width as u32;
        let height = placement.height as u32;

        // Handle different content types
        let data: &[u8] = match content {
            SwashContent::Mask => glyph_data,
            SwashContent::SubpixelMask => {
                // For subpixel, we'd need to convert RGB to grayscale
                // For now, skip subpixel rendering
                return GlyphEntry {
                    uv: [0.0, 0.0, 0.0, 0.0],
                    size: Size::ZERO,
                    offset: Point::ZERO,
                };
            }
            SwashContent::Color => {
                // For color glyphs (emoji), skip for now
                return GlyphEntry {
                    uv: [0.0, 0.0, 0.0, 0.0],
                    size: Size::ZERO,
                    offset: Point::ZERO,
                };
            }
        };

        if width == 0 || height == 0 {
            return GlyphEntry {
                uv: [0.0, 0.0, 0.0, 0.0],
                size: Size::ZERO,
                offset: Point::ZERO,
            };
        }

        // Check if we need to move to next row
        if self.atlas_cursor_x + width > self.atlas_size {
            self.atlas_cursor_x = 0;
            self.atlas_cursor_y += self.atlas_row_height + 1;
            self.atlas_row_height = 0;
        }

        // Check if we've run out of space
        if self.atlas_cursor_y + height > self.atlas_size {
            // Atlas is full - for now, just return empty
            log::warn!("Glyph atlas is full!");
            return GlyphEntry {
                uv: [0.0, 0.0, 0.0, 0.0],
                size: Size::ZERO,
                offset: Point::ZERO,
            };
        }

        // Copy glyph data to atlas
        let x = self.atlas_cursor_x;
        let y = self.atlas_cursor_y;

        for row in 0..height {
            let src_start = (row * width) as usize;
            let src_end = src_start + width as usize;
            let dst_start = ((y + row) * self.atlas_size + x) as usize;

            if src_end <= data.len() && dst_start + width as usize <= self.atlas_data.len() {
                self.atlas_data[dst_start..dst_start + width as usize]
                    .copy_from_slice(&data[src_start..src_end]);
            }
        }

        // Calculate UV coordinates
        let uv = [
            x as f32 / self.atlas_size as f32,
            y as f32 / self.atlas_size as f32,
            (x + width) as f32 / self.atlas_size as f32,
            (y + height) as f32 / self.atlas_size as f32,
        ];

        // Update cursor
        self.atlas_cursor_x += width + 1;
        self.atlas_row_height = self.atlas_row_height.max(height);
        self.dirty = true;

        GlyphEntry {
            uv,
            size: Size::new(width as f32, height as f32),
            offset: Point::new(placement.left as f32, -placement.top as f32),
        }
    }
}
