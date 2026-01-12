use crate::color::Hsla;
use crate::geometry::{Point, Size};
use crate::scene::{GlyphInstance, TextRun};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent,
    Weight,
};
use std::collections::HashMap;

const DEFAULT_FONT_FAMILY: Family<'static> = Family::Name("Square 721");
const MONO_FONT_FAMILY: Family<'static> = Family::Name("Bitstream Vera Sans Mono");

#[cfg(target_arch = "wasm32")]
const DEFAULT_SHAPING: Shaping = Shaping::Basic;
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_SHAPING: Shaping = Shaping::Advanced;

const ATLAS_SIZE: u32 = 2048;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
}

impl FontStyle {
    pub const fn normal() -> Self {
        Self {
            bold: false,
            italic: false,
        }
    }

    pub const fn bold() -> Self {
        Self {
            bold: true,
            italic: false,
        }
    }

    pub const fn italic() -> Self {
        Self {
            bold: false,
            italic: true,
        }
    }

    pub const fn bold_italic() -> Self {
        Self {
            bold: true,
            italic: true,
        }
    }
}

#[derive(Clone, Debug)]
struct GlyphEntry {
    uv: [f32; 4],
    size: Size,
    offset: Point,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct GlyphCacheKey {
    cache_key: CacheKey,
}

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
        let mut font_system = FontSystem::new();

        let square721 = include_bytes!("../../../src/gui/assets/fonts/Square721StdRoman.ttf");
        let regular = include_bytes!("../../../src/gui/assets/fonts/VeraMono.ttf");
        let bold = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Bold.ttf");
        let italic = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Italic.ttf");
        let bold_italic = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Bold-Italic.ttf");

        font_system.db_mut().load_font_data(square721.to_vec());
        font_system.db_mut().load_font_data(regular.to_vec());
        font_system.db_mut().load_font_data(bold.to_vec());
        font_system.db_mut().load_font_data(italic.to_vec());
        font_system.db_mut().load_font_data(bold_italic.to_vec());

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

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        if (self.scale_factor - scale_factor).abs() > 0.001 {
            self.scale_factor = scale_factor;
            self.clear_cache();
        }
    }

    pub fn clear_cache(&mut self) {
        self.glyph_cache.clear();
        self.atlas_data.fill(0);
        self.atlas_cursor_x = 0;
        self.atlas_cursor_y = 0;
        self.atlas_row_height = 0;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }

    /// Measure text width using actual font metrics.
    pub fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        self.measure_styled(text, font_size, FontStyle::default())
    }

    /// Measure styled text width using actual font metrics.
    /// Returns width in logical pixels (matching what layout_styled produces).
    pub fn measure_styled(&mut self, text: &str, font_size: f32, style: FontStyle) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        // Use the SAME shaping as layout to guarantee consistency
        let (line_width, _) = self.shape_text(text, font_size, style);
        line_width
    }

    pub fn measure_size(&mut self, text: &str, font_size: f32, _max_width: Option<f32>) -> Size {
        let width = self.measure(text, font_size);
        // Height in LOGICAL pixels (same as width).
        // Scaling to physical happens at GPU boundary.
        let height = font_size * 1.2;
        Size::new(width, height)
    }

    pub fn layout(&mut self, text: &str, origin: Point, font_size: f32, color: Hsla) -> TextRun {
        self.layout_styled(text, origin, font_size, color, FontStyle::default())
    }

    /// Layout text using monospace font with default style
    pub fn layout_mono(
        &mut self,
        text: &str,
        origin: Point,
        font_size: f32,
        color: Hsla,
    ) -> TextRun {
        self.layout_styled_mono(text, origin, font_size, color, FontStyle::default())
    }

    pub fn layout_styled(
        &mut self,
        text: &str,
        origin: Point,
        font_size: f32,
        color: Hsla,
        style: FontStyle,
    ) -> TextRun {
        // Use shared shaping function
        let (_, glyph_data) = self.shape_text(text, font_size, style);

        let mut text_run = TextRun::new(origin, color, font_size);

        for (cache_key, glyph_x, line_y, glyph_id) in glyph_data {
            let entry = if let Some(entry) = self.glyph_cache.get(&cache_key) {
                entry.clone()
            } else {
                let image_data: Option<(i32, i32, u32, u32, SwashContent, Vec<u8>)> = self
                    .swash_cache
                    .get_image(&mut self.font_system, cache_key.cache_key)
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
                    let placement = cosmic_text::Placement {
                        left,
                        top,
                        width,
                        height,
                    };
                    let entry = self.pack_glyph_data(&placement, content, &data);
                    self.glyph_cache.insert(cache_key, entry.clone());
                    entry
                } else {
                    continue;
                }
            };

            // Convert all coordinates from physical to LOGICAL pixels.
            // - glyph_x, line_y: from cosmic_text shaped at physical_font_size
            // - entry.offset: from swash glyph bitmap placement at physical_font_size
            // - entry.size: glyph bitmap dimensions at physical_font_size
            // All divided by scale_factor to get logical coordinates.
            // Scaling to physical happens at GPU boundary in scene.rs.
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

    /// Internal: Shape text and return both line width and glyph data.
    /// This is the SINGLE SOURCE OF TRUTH for text shaping - both measure() and layout() use this.
    ///
    /// Returns width in LOGICAL pixels for layout calculations.
    /// Glyph data is also stored with positions ready for logical coordinate output.
    /// Scaling to physical pixels happens at the GPU boundary (in scene.rs).
    fn shape_text(
        &mut self,
        text: &str,
        font_size: f32,
        style: FontStyle,
    ) -> (f32, Vec<(GlyphCacheKey, f32, f32, u16)>) {
        if text.is_empty() {
            return (0.0, Vec::new());
        }

        let physical_font_size = font_size * self.scale_factor;
        log::debug!(
            "shape_text: font_size={}, scale_factor={}, physical_font_size={}",
            font_size,
            self.scale_factor,
            physical_font_size
        );
        let metrics = Metrics::new(physical_font_size, physical_font_size * 1.2);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(10000.0), None);

        let attrs = Attrs::new()
            .family(DEFAULT_FONT_FAMILY)
            .weight(if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if style.italic {
                Style::Italic
            } else {
                Style::Normal
            });

        buffer.set_text(&mut self.font_system, text, attrs, DEFAULT_SHAPING);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut glyph_data: Vec<(GlyphCacheKey, f32, f32, u16)> = Vec::new();
        let mut line_width = 0.0f32;

        for run in buffer.layout_runs() {
            // Use line_w which is the actual line width calculated by cosmic_text
            // (this is in physical pixels, we'll convert to logical below)
            line_width = line_width.max(run.line_w);

            for glyph in run.glyphs.iter() {
                // Pass scale_factor=1.0 since we already shaped at physical_font_size.
                // The cache_key determines the rasterization size - using 1.0 means
                // we rasterize at exactly physical_font_size (not physical_font_size * scale_factor).
                let physical_glyph = glyph.physical((0.0, 0.0), 1.0);
                let cache_key = GlyphCacheKey {
                    cache_key: physical_glyph.cache_key,
                };
                // cosmic_text returns glyph.x and line_y in physical pixels (because we used physical_font_size).
                // Store the raw physical values - we'll convert to logical in layout_styled.
                let glyph_x = glyph.x;
                let line_y = run.line_y;

                glyph_data.push((cache_key, glyph_x, line_y, glyph.glyph_id));
            }
        }

        // Convert width from physical to LOGICAL pixels for layout calculations.
        // User code (markdown renderer, etc.) works in logical pixels.
        let logical_width = line_width / self.scale_factor;

        (logical_width, glyph_data)
    }

    /// Shape text using monospace font (Vera Mono)
    fn shape_text_mono(
        &mut self,
        text: &str,
        font_size: f32,
        style: FontStyle,
    ) -> (f32, Vec<(GlyphCacheKey, f32, f32, u16)>) {
        if text.is_empty() {
            return (0.0, Vec::new());
        }

        let physical_font_size = font_size * self.scale_factor;
        let metrics = Metrics::new(physical_font_size, physical_font_size * 1.2);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(10000.0), None);

        let attrs = Attrs::new()
            .family(MONO_FONT_FAMILY)
            .weight(if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if style.italic {
                Style::Italic
            } else {
                Style::Normal
            });

        buffer.set_text(&mut self.font_system, text, attrs, DEFAULT_SHAPING);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut glyph_data: Vec<(GlyphCacheKey, f32, f32, u16)> = Vec::new();
        let mut line_width = 0.0f32;

        for run in buffer.layout_runs() {
            line_width = line_width.max(run.line_w);

            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0.0, 0.0), 1.0);
                let cache_key = GlyphCacheKey {
                    cache_key: physical_glyph.cache_key,
                };
                let glyph_x = glyph.x;
                let line_y = run.line_y;

                glyph_data.push((cache_key, glyph_x, line_y, glyph.glyph_id));
            }
        }

        let logical_width = line_width / self.scale_factor;
        (logical_width, glyph_data)
    }

    /// Layout styled text using monospace font (Vera Mono)
    pub fn layout_styled_mono(
        &mut self,
        text: &str,
        origin: Point,
        font_size: f32,
        color: Hsla,
        style: FontStyle,
    ) -> TextRun {
        let (_, glyph_data) = self.shape_text_mono(text, font_size, style);

        let mut text_run = TextRun::new(origin, color, font_size);

        for (cache_key, glyph_x, line_y, glyph_id) in glyph_data {
            let entry = if let Some(entry) = self.glyph_cache.get(&cache_key) {
                entry.clone()
            } else {
                let image_data: Option<(i32, i32, u32, u32, SwashContent, Vec<u8>)> = self
                    .swash_cache
                    .get_image(&mut self.font_system, cache_key.cache_key)
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
                    let placement = cosmic_text::Placement {
                        left,
                        top,
                        width,
                        height,
                    };
                    let entry = self.pack_glyph_data(&placement, content, &data);
                    self.glyph_cache.insert(cache_key, entry.clone());
                    entry
                } else {
                    continue;
                }
            };

            // Convert all coordinates from physical to LOGICAL pixels
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

    /// Measure styled text width using monospace font (Vera Mono)
    pub fn measure_styled_mono(&mut self, text: &str, font_size: f32, style: FontStyle) -> f32 {
        let (width, _) = self.shape_text_mono(text, font_size, style);
        width
    }

    fn pack_glyph_data(
        &mut self,
        placement: &cosmic_text::Placement,
        content: SwashContent,
        glyph_data: &[u8],
    ) -> GlyphEntry {
        let width = placement.width;
        let height = placement.height;

        let data: &[u8] = match content {
            SwashContent::Mask => glyph_data,
            SwashContent::SubpixelMask | SwashContent::Color => {
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

        if self.atlas_cursor_x + width > self.atlas_size {
            self.atlas_cursor_x = 0;
            self.atlas_cursor_y += self.atlas_row_height + 1;
            self.atlas_row_height = 0;
        }

        if self.atlas_cursor_y + height > self.atlas_size {
            log::warn!("Glyph atlas is full!");
            return GlyphEntry {
                uv: [0.0, 0.0, 0.0, 0.0],
                size: Size::ZERO,
                offset: Point::ZERO,
            };
        }

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

        let uv = [
            x as f32 / self.atlas_size as f32,
            y as f32 / self.atlas_size as f32,
            (x + width) as f32 / self.atlas_size as f32,
            (y + height) as f32 / self.atlas_size as f32,
        ];

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

impl Default for TextSystem {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_style_constructors() {
        let normal = FontStyle::normal();
        assert!(!normal.bold);
        assert!(!normal.italic);

        let bold = FontStyle::bold();
        assert!(bold.bold);
        assert!(!bold.italic);

        let italic = FontStyle::italic();
        assert!(!italic.bold);
        assert!(italic.italic);

        let bold_italic = FontStyle::bold_italic();
        assert!(bold_italic.bold);
        assert!(bold_italic.italic);
    }

    #[test]
    fn test_text_system_creation() {
        let system = TextSystem::new(1.0);
        assert_eq!(system.atlas_size(), 2048);
        assert!(system.is_dirty());
    }

    #[test]
    fn test_measure_text() {
        let mut system = TextSystem::new(1.0);
        let width = system.measure("Hello", 14.0);
        assert!(width > 0.0);
    }

    #[test]
    fn test_measure_size() {
        let mut system = TextSystem::new(1.0);
        let size = system.measure_size("Hello", 14.0, None);
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }

    #[test]
    fn test_layout_text() {
        let mut system = TextSystem::new(1.0);
        let text_run = system.layout("A", Point::new(10.0, 20.0), 14.0, Hsla::white());
        assert!((text_run.font_size - 14.0).abs() < 0.001);
    }

    #[test]
    fn test_scale_factor_change() {
        let mut system = TextSystem::new(1.0);
        system.layout("Test", Point::ZERO, 14.0, Hsla::white());
        system.mark_clean();
        assert!(!system.is_dirty());

        system.set_scale_factor(2.0);
        assert!(system.is_dirty());
    }

    #[test]
    fn test_clear_cache() {
        let mut system = TextSystem::new(1.0);
        system.layout("Test", Point::ZERO, 14.0, Hsla::white());
        system.mark_clean();

        system.clear_cache();
        assert!(system.is_dirty());
    }

    /// CRITICAL: Verify that measure() returns widths that match actual glyph layout.
    /// If this test fails, text spans will overlap when rendered side-by-side.
    #[test]
    fn test_measure_matches_layout_width() {
        let mut system = TextSystem::new(1.0);

        let test_strings = ["Hello", "World!", "Test 123", "a", "ABCDEFGHIJ"];
        let font_size = 14.0;

        for text in test_strings {
            let measured_width = system.measure(text, font_size);
            let text_run = system.layout(text, Point::ZERO, font_size, Hsla::white());

            // Calculate actual width from glyph positions
            let actual_width = text_run
                .glyphs
                .iter()
                .map(|g| g.offset.x + g.size.width)
                .fold(0.0f32, |a, b| a.max(b));

            // Measured width should be >= actual glyph extent (with small tolerance)
            assert!(
                measured_width >= actual_width - 1.0,
                "measure({:?}) = {} but actual glyph width = {}. Text will overlap!",
                text,
                measured_width,
                actual_width
            );
        }
    }

    /// CRITICAL: Verify measure returns LOGICAL pixels (same width regardless of scale_factor).
    /// Layout happens in logical pixels; scaling happens at GPU boundary.
    #[test]
    fn test_measure_at_different_scale_factors() {
        let scales = [1.0, 1.5, 2.0, 2.5];
        let text = "Hello World";
        let font_size = 14.0;

        let mut widths = Vec::new();

        for scale in scales {
            let mut system = TextSystem::new(scale);
            let width = system.measure(text, font_size);
            widths.push(width);

            // Width should always be positive
            assert!(width > 0.0, "Width at scale {} should be positive", scale);
        }

        // LOGICAL width should be the SAME at all scale factors.
        // Scaling to physical happens at the GPU boundary, not in measure().
        let base_width = widths[0];
        for (i, &width) in widths.iter().enumerate() {
            // Allow 5% tolerance for minor hinting variations between different scale factors
            let ratio = width / base_width;
            assert!(
                (ratio - 1.0).abs() < 0.05,
                "Logical width at scale {} ({}) should be ~same as base width ({}). Ratio: {}",
                scales[i],
                width,
                base_width,
                ratio
            );
        }
    }

    /// Test that consecutive spans don't overlap when rendered.
    /// We verify that the measured width correctly advances the cursor,
    /// so subsequent spans start after the previous one ends.
    #[test]
    fn test_consecutive_spans_no_overlap() {
        let mut system = TextSystem::new(2.0); // Use 2x scale to catch scaling bugs

        let spans = ["Hello ", "World ", "Test"];
        let font_size = 14.0;

        // Measure each span and verify total width matches concatenated string
        let total_measured: f32 = spans.iter().map(|s| system.measure(s, font_size)).sum();
        let concatenated = spans.join("");
        let concatenated_width = system.measure(&concatenated, font_size);

        // Total of individual measurements should approximately equal concatenated measurement
        // (exact match isn't required due to kerning, but should be close)
        let ratio = total_measured / concatenated_width;
        assert!(
            (0.95..=1.05).contains(&ratio),
            "Individual span widths ({}) don't match concatenated ({}) - ratio: {}",
            total_measured,
            concatenated_width,
            ratio
        );

        // Verify layout at consecutive positions works correctly
        let mut x = 0.0;
        let mut prev_end = 0.0f32;

        for span in spans {
            let width = system.measure(span, font_size);
            let text_run = system.layout(span, Point::new(x, 0.0), font_size, Hsla::white());

            // First glyph should start at or after where we positioned the text
            if let Some(first_glyph) = text_run.glyphs.first() {
                // Allow for negative offsets (glyphs can extend left of origin)
                let glyph_start = x + first_glyph.offset.x;
                assert!(
                    glyph_start >= prev_end - 2.0, // 2px tolerance
                    "Span {:?} starts at {} which overlaps with previous end {}",
                    span,
                    glyph_start,
                    prev_end
                );
            }

            prev_end = x + width;
            x += width;
        }
    }

    /// Test that measure_styled with a font style returns consistent width.
    /// For monospace fonts like Vera Mono, bold/normal have same width,
    /// but we must use measure_styled consistently with layout_styled.
    #[test]
    fn test_measure_styled_consistency() {
        let mut system = TextSystem::new(2.0);
        let text = "Hello World";
        let font_size = 14.0;

        // Measure with different styles - should all be positive
        let normal_width = system.measure_styled(text, font_size, FontStyle::normal());
        let bold_width = system.measure_styled(text, font_size, FontStyle::bold());
        let italic_width = system.measure_styled(text, font_size, FontStyle::italic());

        assert!(normal_width > 0.0, "Normal width should be positive");
        assert!(bold_width > 0.0, "Bold width should be positive");
        assert!(italic_width > 0.0, "Italic width should be positive");

        // For monospace fonts, all styles should have same width
        // (this validates the font is working correctly)
        assert!(
            (normal_width - bold_width).abs() < 1.0,
            "Monospace font: normal ({}) and bold ({}) should have similar width",
            normal_width,
            bold_width
        );
    }

    /// Test that line height provides adequate spacing.
    #[test]
    fn test_line_height_spacing() {
        let mut system = TextSystem::new(1.0);
        let font_size = 14.0;

        let size = system.measure_size("Hello", font_size, None);

        // Line height should be at least font_size
        assert!(
            size.height >= font_size,
            "Line height {} should be >= font_size {}",
            size.height,
            font_size
        );

        // Line height should provide reasonable spacing (1.2x is standard)
        let expected_line_height = font_size * 1.2;
        assert!(
            (size.height - expected_line_height).abs() < 1.0,
            "Line height {} should be close to {} (font_size * 1.2)",
            size.height,
            expected_line_height
        );
    }

    // ============================================================================
    // CRITICAL: Tests to prevent double-scaling bug from recurring
    // The bug was: passing scale_factor to glyph.physical() when we already
    // shaped at physical_font_size, causing glyphs to be rasterized at
    // physical_font_size * scale_factor (double the intended size).
    // ============================================================================

    /// CRITICAL: Verify glyph sizes are proportional to font size, not double-scaled.
    /// This test catches the double-scaling bug where glyph.physical() was called
    /// with scale_factor when we already shaped at physical_font_size.
    #[test]
    fn test_glyph_size_not_double_scaled() {
        let scale_factor = 2.0;
        let mut system = TextSystem::new(scale_factor);
        let font_size = 14.0;

        let text_run = system.layout("A", Point::ZERO, font_size, Hsla::white());
        assert!(
            !text_run.glyphs.is_empty(),
            "Should have at least one glyph"
        );

        let glyph = &text_run.glyphs[0];

        // For monospace font at 14px logical, glyph width should be approximately
        // font_size * 0.6 = 8.4 logical pixels, NOT 16.8 (which would be double-scaled).
        // Allow tolerance for font metrics variation.
        let expected_width = font_size * 0.6;
        let max_reasonable_width = font_size * 1.0; // Never more than font_size

        assert!(
            glyph.size.width <= max_reasonable_width,
            "Glyph width {} exceeds maximum reasonable width {} for {}px font. \
            This suggests double-scaling bug in glyph.physical() call!",
            glyph.size.width,
            max_reasonable_width,
            font_size
        );

        // Width should be in reasonable range for monospace
        assert!(
            glyph.size.width >= expected_width * 0.5 && glyph.size.width <= expected_width * 2.0,
            "Glyph width {} is not in expected range [{}, {}] for {}px monospace font",
            glyph.size.width,
            expected_width * 0.5,
            expected_width * 2.0,
            font_size
        );
    }

    /// CRITICAL: Verify glyph spacing approximately equals glyph width for monospace.
    /// If glyphs overlap significantly, the double-scaling bug has returned.
    #[test]
    fn test_glyph_spacing_matches_width() {
        let scale_factor = 2.0;
        let mut system = TextSystem::new(scale_factor);
        let font_size = 14.0;

        let text_run = system.layout("AA", Point::ZERO, font_size, Hsla::white());
        assert_eq!(text_run.glyphs.len(), 2, "Should have exactly 2 glyphs");

        let glyph_0 = &text_run.glyphs[0];
        let glyph_1 = &text_run.glyphs[1];

        // Calculate spacing between glyphs
        let spacing = glyph_1.offset.x - glyph_0.offset.x;

        // For monospace font, spacing should approximately equal glyph width
        // If spacing << width, glyphs will overlap (indicates double-scaling bug)
        let width = glyph_0.size.width;

        assert!(
            spacing >= width * 0.8,
            "Glyph spacing {} is less than 80% of glyph width {}. \
            This indicates glyphs will overlap! Double-scaling bug likely.",
            spacing,
            width
        );

        // Spacing should not be much larger than width either
        assert!(
            spacing <= width * 1.5,
            "Glyph spacing {} is more than 150% of glyph width {}. \
            This indicates unexpected gaps between glyphs.",
            spacing,
            width
        );
    }

    /// CRITICAL: Verify consistent sizing across different scale factors.
    /// The LOGICAL glyph size should be approximately the same regardless of scale_factor.
    /// Note: Some variation is expected due to font hinting at different sizes.
    #[test]
    fn test_glyph_size_consistent_across_scale_factors() {
        let font_size = 14.0;

        let mut system_1x = TextSystem::new(1.0);
        let mut system_2x = TextSystem::new(2.0);
        let mut system_3x = TextSystem::new(3.0);

        let run_1x = system_1x.layout("A", Point::ZERO, font_size, Hsla::white());
        let run_2x = system_2x.layout("A", Point::ZERO, font_size, Hsla::white());
        let run_3x = system_3x.layout("A", Point::ZERO, font_size, Hsla::white());

        assert!(!run_1x.glyphs.is_empty());
        assert!(!run_2x.glyphs.is_empty());
        assert!(!run_3x.glyphs.is_empty());

        let size_1x = run_1x.glyphs[0].size.width;
        let size_2x = run_2x.glyphs[0].size.width;
        let size_3x = run_3x.glyphs[0].size.width;

        // Logical sizes should be approximately equal
        // Allow 25% tolerance for hinting variations at different physical sizes
        let tolerance = 0.25;
        assert!(
            (size_2x / size_1x - 1.0).abs() < tolerance,
            "Glyph size at 2x ({}) differs too much from 1x ({}). Ratio: {}. \
            LOGICAL size should be roughly consistent across scale factors!",
            size_2x,
            size_1x,
            size_2x / size_1x
        );
        assert!(
            (size_3x / size_1x - 1.0).abs() < tolerance,
            "Glyph size at 3x ({}) differs too much from 1x ({}). Ratio: {}. \
            LOGICAL size should be roughly consistent across scale factors!",
            size_3x,
            size_1x,
            size_3x / size_1x
        );
    }

    /// CRITICAL: Verify that measured text width contains all glyphs without overlap.
    /// The rightmost glyph's extent should not exceed the measured width.
    #[test]
    fn test_measured_width_contains_all_glyphs() {
        let scale_factor = 2.0;
        let mut system = TextSystem::new(scale_factor);
        let font_size = 14.0;
        let text = "AAAA";

        let measured_width = system.measure(text, font_size);
        let text_run = system.layout(text, Point::ZERO, font_size, Hsla::white());

        // Find the rightmost extent of any glyph
        let rightmost_extent = text_run
            .glyphs
            .iter()
            .map(|g| g.offset.x + g.size.width)
            .fold(0.0f32, |a, b| a.max(b));

        // Measured width should be >= rightmost glyph extent
        // (with small tolerance for rounding)
        assert!(
            measured_width >= rightmost_extent - 1.0,
            "Measured width {} is less than rightmost glyph extent {}. \
            This means glyphs extend beyond their allocated space!",
            measured_width,
            rightmost_extent
        );

        // Measured width should not be drastically larger than glyph extent
        assert!(
            measured_width <= rightmost_extent * 1.2,
            "Measured width {} is much larger than rightmost glyph extent {}. \
            This suggests measurement inconsistency.",
            measured_width,
            rightmost_extent
        );
    }

    /// CRITICAL: Verify no overlap when rendering consecutive text spans.
    /// This simulates what markdown rendering does with styled text.
    #[test]
    fn test_consecutive_text_spans_no_overlap() {
        let scale_factor = 2.0;
        let mut system = TextSystem::new(scale_factor);
        let font_size = 14.0;

        // Simulate rendering "AAA" then "BBB" consecutively
        let span1 = "AAA";
        let span2 = "BBB";

        let width1 = system.measure(span1, font_size);
        let run1 = system.layout(span1, Point::new(0.0, 0.0), font_size, Hsla::white());
        let run2 = system.layout(span2, Point::new(width1, 0.0), font_size, Hsla::white());

        // Find the rightmost ABSOLUTE extent of span1 (origin + offset + width)
        let span1_right = run1
            .glyphs
            .iter()
            .map(|g| run1.origin.x + g.offset.x + g.size.width)
            .fold(0.0f32, |a, b| a.max(b));

        // Find the leftmost ABSOLUTE position of span2 (origin + offset)
        let span2_left = run2
            .glyphs
            .iter()
            .map(|g| run2.origin.x + g.offset.x)
            .fold(f32::MAX, |a, b| a.min(b));

        // Span2 should start at or after span1 ends (allow 2px tolerance for kerning)
        assert!(
            span2_left >= span1_right - 2.0,
            "Consecutive spans overlap! Span1 ends at {}, span2 starts at {}. \
            This will cause text rendering issues!",
            span1_right,
            span2_left
        );
    }

    /// Verify that different font sizes produce proportionally sized glyphs.
    #[test]
    fn test_font_size_scaling() {
        let mut system = TextSystem::new(2.0);

        let run_14 = system.layout("A", Point::ZERO, 14.0, Hsla::white());
        let run_28 = system.layout("A", Point::ZERO, 28.0, Hsla::white());

        assert!(!run_14.glyphs.is_empty());
        assert!(!run_28.glyphs.is_empty());

        let size_14 = run_14.glyphs[0].size.width;
        let size_28 = run_28.glyphs[0].size.width;

        // 28px should be approximately 2x the size of 14px
        let ratio = size_28 / size_14;
        assert!(
            (ratio - 2.0).abs() < 0.3,
            "28px glyph ({}) should be ~2x 14px glyph ({}). Actual ratio: {}",
            size_28,
            size_14,
            ratio
        );
    }
}
