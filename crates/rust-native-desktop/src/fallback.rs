//! Faces for the characters Paper Mono lacks.
//!
//! Paper Mono is the one bundled face, and it has no CJK, emoji, Greek,
//! Cyrillic, or some symbols, such as the check mark and the bitcoin sign.
//! A platform text engine draws those from its own fallback faces; this
//! adapter has none, so [`face_for`] finds the first of this computer's fonts
//! that has the character. Nothing here is bundled: a missing file is
//! skipped, and a character no file has draws as Paper Mono's missing glyph.
//! The shaper measures such a character at an estimate
//! ([`rust_native::layout::shape::fallback_em`]), and the painter centers the
//! fallback glyph in that width, so layout never depends on which fonts a
//! computer has.

use std::sync::OnceLock;
use swash::FontRef;

/// Font files tried in order, with the face index in a collection. The
/// names are this computer's files, not faces Rust Native draws text in;
/// lines that name one carry the marker `scripts/check-fonts.sh` skips.
const FILES: &[(&str, u32)] = &[
    // macOS.
    ("/System/Library/Fonts/SFNSMono.ttf", 0),
    ("/System/Library/Fonts/Menlo.ttc", 0), // check-fonts: allow
    ("/System/Library/Fonts/Apple Symbols.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0), // check-fonts: allow
    ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
    ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
    // Linux.
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0), // check-fonts: allow
    ("/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
    // Windows.
    ("C:\\Windows\\Fonts\\seguisym.ttf", 0),
    ("C:\\Windows\\Fonts\\msyh.ttc", 0),
];

/// Each file's bytes, read once per process when a character first needs
/// it. The bytes live as long as the process, as the bundled face does.
static LOADED: [OnceLock<Option<&'static [u8]>>; FILES.len()] =
    [const { OnceLock::new() }; FILES.len()];

/// The fallback face that has `ch`, as its index in the list and the face.
pub fn face_for(ch: char) -> Option<(usize, FontRef<'static>)> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    for (index, (path, face)) in FILES.iter().enumerate() {
        let bytes = LOADED[index].get_or_init(|| {
            std::fs::read(path)
                .ok()
                .map(|bytes| &*Box::leak(bytes.into_boxed_slice()))
        });
        let Some(bytes) = bytes else {
            continue;
        };
        if let Some(font) = FontRef::from_index(bytes, *face as usize)
            && font.charmap().map(ch) != 0
        {
            return Some((index, font));
        }
    }
    None
}

/// The fallback face at `index` in the list, once [`face_for`] has loaded it.
pub fn face_for_index(index: usize) -> Option<FontRef<'static>> {
    let bytes = (*LOADED.get(index)?.get()?)?;
    FontRef::from_index(bytes, FILES[index].1 as usize)
}
