//! Paper Mono for native surfaces, web monospace content, and the web logo.
//!
//! The font files live in `fonts/` beside the SIL Open Font License 1.1
//! (`fonts/OFL.txt`). Rust surfaces read them from here: Rust Native shapes
//! and paints with [`VARIABLE`], the Verse atlas rasterizes it, and HTML
//! pages serve [`WOFF2`] through [`font_face`]. The phone hosts and CoderOS
//! bundle the static weights from the same directory.
//!
//! Paper Mono has no italic, so italic text is drawn upright.
//! OpenAgents web pages use a bundled sans face for normal text.
//! `scripts/check-fonts.sh` rejects fonts outside their admitted scopes.

#![forbid(unsafe_code)]

/// The family name every face reports.
pub const FAMILY: &str = "Paper Mono";

/// The CSS `font-family` value: Paper Mono, then the generic monospace.
pub const CSS_STACK: &str = "\"Paper Mono\",monospace";

/// The variable TrueType face, `wght` 100 to 800, default 400.
pub const VARIABLE: &[u8] = include_bytes!("../fonts/PaperMono-Variable.ttf");

/// The variable face as WOFF2, for web pages.
pub const WOFF2: &[u8] = include_bytes!("../fonts/PaperMono-Variable.woff2");

/// The path under an origin where a site serves [`WOFF2`].
pub const WOFF2_PATH: &str = "/fonts/PaperMono-Variable.woff2";

/// The lightest and heaviest `wght` values the variable face draws.
pub const WEIGHTS: std::ops::RangeInclusive<f32> = 100.0..=800.0;

/// A CSS `@font-face` rule for [`WOFF2`] served at `url`, covering every
/// weight the variable face draws. `url` is placed in the rule as given, so
/// it must not hold a double quote.
#[must_use]
pub fn font_face(url: &str) -> String {
    format!(
        "@font-face{{font-family:\"Paper Mono\";src:url(\"{url}\") format(\"woff2\");\
         font-weight:100 800;font-style:normal;font-display:swap}}"
    )
}

/// The [`font_face`] rule with [`WOFF2`] inlined as a `data:` URI, for a
/// page no route serves the font beside, such as a file written to disk or
/// a one-off local callback page. It adds about 71 KB to the page.
#[must_use]
pub fn font_face_inline() -> String {
    font_face(&format!("data:font/woff2;base64,{}", base64(WOFF2)))
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = match chunk {
            [a, b, c] => u32::from(*a) << 16 | u32::from(*b) << 8 | u32::from(*c),
            [a, b] => u32::from(*a) << 16 | u32::from(*b) << 8,
            [a] => u32::from(*a) << 16,
            _ => unreachable!("chunks(3) yields one to three bytes"),
        };
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * index)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_faces_are_paper_mono() {
        // TrueType and WOFF2 signatures.
        assert_eq!(&VARIABLE[..4], &[0, 1, 0, 0]);
        assert_eq!(&WOFF2[..4], b"wOF2");
        let family = b"Paper Mono"
            .iter()
            .flat_map(|byte| [0, *byte])
            .collect::<Vec<u8>>();
        assert!(VARIABLE.windows(family.len()).any(|w| w == family));
    }

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn the_rules_name_the_family_and_the_source() {
        let rule = font_face(WOFF2_PATH);
        assert!(rule.starts_with("@font-face{font-family:\"Paper Mono\";"));
        assert!(rule.contains("url(\"/fonts/PaperMono-Variable.woff2\") format(\"woff2\")"));
        assert!(font_face_inline().contains("url(\"data:font/woff2;base64,d09GMg"));
    }
}
