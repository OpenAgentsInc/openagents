//! This computer's fallback faces: the fonts a character the drawing faces
//! lack is drawn from, and, on Linux and Windows, measured from (#11304).
//!
//! On a Mac the shaper measures such a character at the width CoreText's
//! fallback face gives it (`shape/fallback.rs`), since CoreText is what the
//! other Apple surfaces lay out with, and a painter centers the glyph from
//! here in that width. On Linux and Windows nothing else lays the text out,
//! so the shaper measures the glyphs from the same face the painter draws,
//! and measuring matches drawing glyph for glyph ([`FallbackWidths`]).
//!
//! Which faces:
//! - Linux: fontconfig's fallback order, the sorted font set `fc-match -s`
//!   gives for the emoji, sans-serif and monospace families, with each (check-fonts: allow)
//!   face's character set, read once per process. That is the order Pango
//!   and other fontconfig clients fall back in (Noto Color Emoji, Noto Sans
//!   CJK, DejaVu, ...). Without `fc-match`, the usual distribution paths. (check-fonts: allow)
//! - Windows: the faces DirectWrite falls back to for these scripts: Segoe
//!   UI Emoji, Segoe UI and Segoe UI Symbol, Microsoft YaHei, Malgun (check-fonts: allow)
//!   Gothic, Yu Gothic.
//! - macOS: system faces for symbols, Han and Hangul.
//!
//! Nothing here is bundled: a missing file is skipped, and a character no
//! face has is measured from the CoreText table and drawn as the drawing
//! face's missing glyph. Lines that name a face carry the marker
//! `scripts/check-fonts.sh` skips.

use super::{Fallback, FontSpec};
use std::sync::OnceLock;
use swash::FontRef;
use swash::shape::ShapeContext;

/// How the shaper measures a character the drawing faces lack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackWidths {
    /// CoreText's widths measured on a Mac (`shape/fallback.rs`), whatever
    /// fonts this computer has.
    CoreText,
    /// The glyphs of this computer's fallback face ([`fallback_glyphs`]),
    /// as the painter draws them; CoreText's width when no face has it.
    System,
}

impl FallbackWidths {
    /// This platform's: CoreText's on Apple platforms, where CoreText lays
    /// out the other surfaces, and the system's fallback faces on Linux and
    /// Windows.
    #[must_use]
    pub const fn platform() -> Self {
        if cfg!(any(target_os = "linux", target_os = "windows")) {
            Self::System
        } else {
            Self::CoreText
        }
    }
}

/// A grapheme cluster shaped in a fallback face.
#[derive(Clone, Debug, PartialEq)]
pub struct FallbackGlyphs {
    /// The face's index among this computer's fallback faces
    /// ([`face_at`]).
    pub face: usize,
    /// Each glyph's id and its offset from the cluster's start, in points.
    pub glyphs: Vec<(u16, f32, f32)>,
    /// The glyphs' advance together, in points.
    pub advance: f64,
}

/// One candidate face.
struct Entry {
    path: String,
    index: u32,
    /// The characters it has, as sorted inclusive ranges, when fontconfig
    /// said; otherwise read from the face's character map once loaded.
    charset: Option<Vec<(u32, u32)>>,
    /// Whether it is a color emoji face, tried first for emoji.
    emoji: bool,
}

/// The candidates, in the order to try: one list per [`Fallback`], plus
/// the emoji faces, each holding indexes into [`Faces::entries`].
struct Faces {
    entries: Vec<Entry>,
    emoji: Vec<usize>,
    proportional: Vec<usize>,
    monospace: Vec<usize>,
    loaded: Vec<OnceLock<Option<&'static [u8]>>>,
}

static FACES: OnceLock<Faces> = OnceLock::new();

fn faces() -> &'static Faces {
    FACES.get_or_init(|| {
        #[cfg(target_os = "linux")]
        if let Some(faces) = fontconfig() {
            return faces;
        }
        Faces::from_files(FILES)
    })
}

/// Font files tried in order where fontconfig is not, with the face index
/// in a collection and whether the face is for emoji presentation.
#[cfg(target_os = "macos")]
const FILES: &[(&str, u32, bool)] = &[
    ("/System/Library/Fonts/SFNSMono.ttf", 0, false),
    ("/System/Library/Fonts/Menlo.ttc", 0, false), // check-fonts: allow
    ("/System/Library/Fonts/Apple Symbols.ttf", 0, false),
    (
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", // check-fonts: allow
        0,
        false,
    ),
    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0, false),
    ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0, false),
];

/// The faces DirectWrite falls back to on Windows: emoji, symbols and
/// Latin-script letters, Han, Hangul, then kana.
#[cfg(target_os = "windows")]
const FILES: &[(&str, u32, bool)] = &[
    (r"C:\Windows\Fonts\seguiemj.ttf", 0, true),
    (r"C:\Windows\Fonts\segoeui.ttf", 0, false), // check-fonts: allow
    (r"C:\Windows\Fonts\seguisym.ttf", 0, false),
    (r"C:\Windows\Fonts\msyh.ttc", 0, false),
    (r"C:\Windows\Fonts\malgun.ttf", 0, false),
    (r"C:\Windows\Fonts\YuGothM.ttc", 0, false),
];

/// Distribution paths, for a Linux without `fc-match`.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const FILES: &[(&str, u32, bool)] = &[
    ("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf", 0, true),
    ("/usr/share/fonts/noto/NotoColorEmoji.ttf", 0, true),
    (
        "/usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf",
        0,
        true,
    ),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0, false), // check-fonts: allow
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", 0, false),             // check-fonts: allow
    ("/usr/share/fonts/dejavu/DejaVuSans.ttf", 0, false),          // check-fonts: allow
    (
        "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
        0,
        false,
    ),
    (
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        0,
        false,
    ),
    (
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        0,
        false,
    ),
    (
        "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc",
        0,
        false,
    ),
];

impl Faces {
    fn from_files(files: &[(&str, u32, bool)]) -> Self {
        let entries: Vec<Entry> = files
            .iter()
            .map(|(path, index, emoji)| Entry {
                path: (*path).to_owned(),
                index: *index,
                charset: None,
                emoji: *emoji,
            })
            .collect();
        // Emoji faces go first for emoji presentation, and last otherwise.
        let emoji: Vec<usize> = (0..entries.len()).filter(|i| entries[*i].emoji).collect();
        let all: Vec<usize> = (0..entries.len())
            .filter(|i| !entries[*i].emoji)
            .chain(emoji.iter().copied())
            .collect();
        Self::new(emoji, all.clone(), all, entries)
    }

    fn new(
        emoji: Vec<usize>,
        proportional: Vec<usize>,
        monospace: Vec<usize>,
        entries: Vec<Entry>,
    ) -> Self {
        let loaded = (0..entries.len()).map(|_| OnceLock::new()).collect();
        Self {
            entries,
            emoji,
            proportional,
            monospace,
            loaded,
        }
    }

    /// The face at `index`, reading its file the first time.
    fn load(&self, index: usize) -> Option<FontRef<'static>> {
        let entry = self.entries.get(index)?;
        let bytes = self.loaded[index].get_or_init(|| {
            std::fs::read(&entry.path)
                .ok()
                .map(|bytes| &*Box::leak(bytes.into_boxed_slice()))
        });
        FontRef::from_index((*bytes)?, entry.index as usize)
    }

    /// Whether the face at `index` has `ch`, without reading the file when
    /// fontconfig gave its character set.
    fn has(&self, index: usize, ch: char) -> bool {
        let code = u32::from(ch);
        match &self.entries[index].charset {
            Some(ranges) => {
                let at = ranges.partition_point(|(_, last)| *last < code);
                ranges.get(at).is_some_and(|(first, _)| *first <= code)
                    && self.load(index).is_some()
            }
            None => self
                .load(index)
                .is_some_and(|face| face.charmap().map(ch) != 0),
        }
    }
}

/// fontconfig's fallback order for the emoji, sans-serif and monospace (check-fonts: allow)
/// families, from `fc-match -s`, with each face's character set.
#[cfg(target_os = "linux")]
fn fontconfig() -> Option<Faces> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut lists = Vec::new();
    let families = ["emoji", "sans-serif", "monospace"]; // check-fonts: allow
    for family in families {
        let output = std::process::Command::new("fc-match")
            .args([
                "-s",
                "-f",
                "%{file}\t%{index}\t%{color}\t%{charset}\n",
                family,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut list = Vec::new();
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(path), Some(index)) = (fields.next(), fields.next()) else {
                continue;
            };
            // A variable face's named instance is in the high bits.
            let Ok(index) = index.parse::<u32>().map(|index| index & 0xffff) else {
                continue;
            };
            let color = fields.next() == Some("True");
            let charset = parse_charset(fields.next().unwrap_or(""));
            let at = match entries
                .iter()
                .position(|e| e.path == path && e.index == index)
            {
                Some(at) => at,
                None => {
                    entries.push(Entry {
                        path: path.to_owned(),
                        index,
                        charset: Some(charset),
                        emoji: color,
                    });
                    entries.len() - 1
                }
            };
            list.push(at);
        }
        lists.push(list);
    }
    let monospace = lists.pop()?;
    let proportional = lists.pop()?;
    let mut emoji = lists.pop()?;
    // Only color faces go first for emoji; the rest of that list is the
    // sans-serif order again. (check-fonts: allow)
    emoji.retain(|at| entries[*at].emoji);
    (!proportional.is_empty()).then(|| Faces::new(emoji, proportional, monospace, entries))
}

/// A fontconfig character set: hexadecimal code points and ranges.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_charset(text: &str) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = text
        .split_whitespace()
        .filter_map(|item| {
            let (first, last) = item.split_once('-').unwrap_or((item, item));
            Some((
                u32::from_str_radix(first, 16).ok()?,
                u32::from_str_radix(last, 16).ok()?,
            ))
        })
        .collect();
    ranges.sort_unstable();
    ranges
}

/// The fallback face that draws `cluster`, a grapheme cluster the drawing
/// face lacks, as its index ([`face_at`]) and the face: an emoji face first
/// when the cluster presents as emoji, then the list for `fallback`.
#[must_use]
pub fn face_for(cluster: &str, fallback: Fallback) -> Option<(usize, FontRef<'static>)> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let first = cluster.chars().next()?;
    let faces = faces();
    let list = match fallback {
        Fallback::Proportional => &faces.proportional,
        Fallback::Monospace => &faces.monospace,
    };
    let emoji = super::presents_as_emoji(cluster, fallback);
    emoji
        .then_some(&faces.emoji)
        .into_iter()
        .flatten()
        .chain(list)
        .find(|index| faces.has(**index, first))
        .and_then(|index| Some((*index, faces.load(*index)?)))
}

/// The fallback face at `index`, once [`face_for`] has found it.
#[must_use]
pub fn face_at(index: usize) -> Option<FontRef<'static>> {
    faces().load(index)
}

/// `cluster`, a grapheme cluster the drawing face lacks, shaped in its
/// fallback face ([`face_for`]) at `spec`'s size and weight, or `None` when
/// no face on this computer has it.
pub fn fallback_glyphs(
    context: &mut ShapeContext,
    cluster: &str,
    spec: &FontSpec,
) -> Option<FallbackGlyphs> {
    let (face, font) = face_for(cluster, spec.fallback)?;
    let upem = f64::from(font.metrics(&[]).units_per_em.max(1));
    let scale = f64::from(spec.size) / upem;
    let mut shaper = context
        .builder(font)
        .size(0.0)
        .variations([("wght", spec.weight)])
        .build();
    shaper.add_str(cluster);
    let mut glyphs = Vec::new();
    let mut advance = 0.0f64;
    shaper.shape_with(|shaped| {
        for glyph in shaped.glyphs {
            glyphs.push((
                glyph.id,
                ((advance + f64::from(glyph.x)) * scale) as f32,
                (f64::from(glyph.y) * scale) as f32,
            ));
            advance += f64::from(glyph.advance);
        }
    });
    // A selector or joiner the face has no glyph for is drawn as nothing,
    // in the width the face shaped it.
    glyphs.retain(|glyph| glyph.0 != 0);
    (!glyphs.is_empty()).then(|| FallbackGlyphs {
        face,
        glyphs,
        advance: advance * scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fontconfig_charset_reads_as_sorted_ranges() {
        assert_eq!(
            parse_charset("20-7e a0-17f 4e00-9fff 1f600"),
            vec![
                (0x20, 0x7e),
                (0xa0, 0x17f),
                (0x4e00, 0x9fff),
                (0x1f600, 0x1f600)
            ]
        );
        assert_eq!(parse_charset(""), vec![]);
    }
}
