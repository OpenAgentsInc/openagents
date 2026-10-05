//! Glyphs the atlas does not carry, rasterized on demand into its reserved
//! rows: Fira Mono's own glyphs beyond the prebuilt set first, then this
//! computer's fonts for symbols, CJK, and emoji. Color emoji become a
//! coverage mask, so they draw in the pane's white ladder like any text.
//!
//! Box drawing, block elements, braille, and powerline separators never
//! reach here; `draw` makes them from rectangles and triangles, so they
//! join cleanly across cells.

use std::collections::HashSet;
use std::path::PathBuf;

use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;

use crate::ui::Atlas;

/// Fonts tried after Fira Mono, in order, with the face index in a
/// collection. Missing files are skipped.
const FONTS: &[(&str, u32)] = &[
    ("/System/Library/Fonts/Menlo.ttc", 0),
    ("/System/Library/Fonts/Apple Symbols.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
    ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
    ("/System/Library/Fonts/Apple Braille.ttf", 0),
    ("/System/Library/Fonts/Apple Color Emoji.ttc", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
    (
        "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf", 0),
    ("/usr/share/fonts/noto/NotoColorEmoji.ttf", 0),
];

enum Data {
    Static(&'static [u8]),
    Mapped(memmap2::Mmap),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Static(bytes) => bytes,
            Data::Mapped(map) => map,
        }
    }
}

struct Font {
    path: Option<PathBuf>,
    index: u32,
    data: Option<Data>,
    failed: bool,
}

impl Font {
    fn bytes(&mut self) -> Option<&[u8]> {
        if self.data.is_none() && !self.failed {
            let path = self.path.as_ref()?;
            let file = std::fs::File::open(path).ok();
            // SAFETY: system font files are read-only and not truncated
            // while Verse runs; a font that changed under the map could at
            // worst draw a wrong glyph.
            let map = file.and_then(|file| unsafe { memmap2::Mmap::map(&file) }.ok());
            match map {
                Some(map) => self.data = Some(Data::Mapped(map)),
                None => self.failed = true,
            }
        }
        self.data.as_ref().map(Data::bytes)
    }
}

/// The fallback chain and what it could not find.
pub struct Fallback {
    fonts: Vec<Font>,
    missing: HashSet<char>,
    context: ScaleContext,
}

impl std::fmt::Debug for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fallback")
            .field("fonts", &self.fonts.len())
            .field("missing", &self.missing.len())
            .finish_non_exhaustive()
    }
}

impl Default for Fallback {
    fn default() -> Self {
        Fallback::new()
    }
}

impl Fallback {
    /// Fira Mono, then the fonts of [`FONTS`] this computer has.
    #[must_use]
    pub fn new() -> Self {
        let mut fonts = vec![Font {
            path: None,
            index: 0,
            data: Some(Data::Static(crate::ui::MONO_FONT)),
            failed: false,
        }];
        fonts.extend(
            FONTS
                .iter()
                .map(|(path, index)| (PathBuf::from(path), *index))
                .filter(|(path, _)| path.is_file())
                .map(|(path, index)| Font {
                    path: Some(path),
                    index,
                    data: None,
                    failed: false,
                }),
        );
        Fallback {
            fonts,
            missing: HashSet::new(),
            context: ScaleContext::new(),
        }
    }

    /// Makes sure `atlas` can draw `c`, rasterizing it from the first font
    /// that has it. Returns false when no font has it or the atlas has no
    /// room; a character no font has is not looked up again.
    pub fn ensure(&mut self, atlas: &mut Atlas, c: char) -> bool {
        if atlas.has_glyph(c) {
            return true;
        }
        if self.missing.contains(&c) {
            return false;
        }
        let px = atlas.raster_px;
        for font in &mut self.fonts {
            let index = font.index;
            let Some(bytes) = font.bytes() else {
                continue;
            };
            let Some(face) = FontRef::from_index(bytes, index as usize) else {
                continue;
            };
            let id = face.charmap().map(c);
            if id == 0 {
                continue;
            }
            let mut scaler = self.context.builder(face).size(px).hint(true).build();
            let Some(image) =
                Render::new(&[Source::ColorBitmap(StrikeWith::BestFit), Source::Outline])
                    .format(Format::Alpha)
                    .render(&mut scaler, id)
            else {
                continue;
            };
            let (w, h) = (image.placement.width, image.placement.height);
            let coverage: Vec<u8> = match image.content {
                Content::Mask => image.data,
                // A color glyph's light, kept as coverage: the pane tints it.
                Content::Color => image
                    .data
                    .chunks_exact(4)
                    .map(|p| {
                        let light = (0.2126 * f32::from(p[0])
                            + 0.7152 * f32::from(p[1])
                            + 0.0722 * f32::from(p[2]))
                            / 255.0;
                        (f32::from(p[3]) * (0.3 + 0.7 * light)).round() as u8
                    })
                    .collect(),
                Content::SubpixelMask => image.data.chunks_exact(4).map(|p| p[1]).collect(),
            };
            let advance = face.glyph_metrics(&[]).scale(px).advance_width(id);
            let left = image.placement.left;
            let top = image.placement.top;
            if atlas.insert_glyph(c, &coverage, w, h, left, top, advance) {
                return true;
            }
            break;
        }
        self.missing.insert(c);
        false
    }
}
