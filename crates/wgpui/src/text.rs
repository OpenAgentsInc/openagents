use crate::color::Hsla;
use crate::geometry::{Point, Size};
use crate::scene::{GlyphInstance, TextRun};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent,
    Weight,
};
use std::collections::HashMap;

const DEFAULT_FONT_FAMILY: Family<'static> = Family::Name("Bitstream Vera Sans Mono");

#[cfg(target_arch = "wasm32")]
const DEFAULT_SHAPING: Shaping = Shaping::Basic;
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_SHAPING: Shaping = Shaping::Advanced;

const ATLAS_SIZE: u32 = 1024;

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
        let font_system = {
            use cosmic_text::fontdb;
            let mut db = fontdb::Database::new();

            let regular = include_bytes!("../../../src/gui/assets/fonts/VeraMono.ttf");
            let bold = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Bold.ttf");
            let italic = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Italic.ttf");
            let bold_italic = include_bytes!("../../../src/gui/assets/fonts/VeraMono-Bold-Italic.ttf");

            db.load_font_data(regular.to_vec());
            db.load_font_data(bold.to_vec());
            db.load_font_data(italic.to_vec());
            db.load_font_data(bold_italic.to_vec());

            FontSystem::new_with_locale_and_db("en-US".to_string(), db)
        };

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

    const CHAR_WIDTH_RATIO: f32 = 0.6;

    pub fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        let char_count = text.chars().count() as f32;
        char_count * font_size * Self::CHAR_WIDTH_RATIO
    }

    pub fn measure_size(&mut self, text: &str, font_size: f32, _max_width: Option<f32>) -> Size {
        let char_count = text.chars().count() as f32;
        let width = char_count * font_size * Self::CHAR_WIDTH_RATIO;
        let height = font_size * 1.2;
        Size::new(width, height)
    }

    pub fn layout(&mut self, text: &str, origin: Point, font_size: f32, color: Hsla) -> TextRun {
        self.layout_styled(text, origin, font_size, color, FontStyle::default())
    }

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

        let mut text_run = TextRun::new(origin, color, font_size);
        let mut glyph_data: Vec<(GlyphCacheKey, f32, f32, u16)> = Vec::new();

        for run in buffer.layout_runs() {
            let mut current_x = 0.0f32;
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0.0, 0.0), self.scale_factor);
                let cache_key = GlyphCacheKey {
                    cache_key: physical_glyph.cache_key,
                };
                glyph_data.push((cache_key, current_x, run.line_y, glyph.glyph_id as u16));
                let advance = physical_font_size * Self::CHAR_WIDTH_RATIO;
                current_x += advance;
            }
        }

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

    fn pack_glyph_data(
        &mut self,
        placement: &cosmic_text::Placement,
        content: SwashContent,
        glyph_data: &[u8],
    ) -> GlyphEntry {
        let width = placement.width as u32;
        let height = placement.height as u32;

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
        assert_eq!(system.atlas_size(), 1024);
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
}
