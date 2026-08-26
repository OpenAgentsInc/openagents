//! Display-width helpers shared by the wrapping and line-fitting passes.
//!
//! Ported from xAI's `grok-build` (`crates/codegen/xai-grok-pager-render/src/util.rs`),
//! Apache-2.0. See `LICENSE-APACHE-xai` beside this file.

use std::borrow::Cow;
use unicode_width::UnicodeWidthChar;

/// Truncate `s` so that it renders in at most `max_width` display columns,
/// appending `…` when a truncation actually happened.
///
/// Width is measured with `unicode-width`, so CJK and other wide glyphs count
/// as the two columns the terminal actually paints.
pub fn truncate_to_width(s: &str, max_width: usize) -> Cow<'_, str> {
    if byte_offset_at_width(s, max_width) == s.len() {
        return Cow::Borrowed(s);
    }
    if max_width == 0 {
        return Cow::Borrowed("");
    }
    let end = byte_offset_at_width(s, max_width - 1);
    Cow::Owned(format!("{}…", &s[..end]))
}

/// Byte offset of the first character that would push `s` past `max_width`
/// display columns. Returns `s.len()` when the whole string fits.
pub fn byte_offset_at_width(s: &str, max_width: usize) -> usize {
    let mut width = 0;
    for (i, ch) in s.char_indices() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > max_width {
            return i;
        }
        width += cw;
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_counts_wide_glyphs_as_two_columns() {
        // Each CJK ideograph is 3 bytes wide but 2 display columns.
        assert_eq!(byte_offset_at_width("日本語", 6), "日本語".len());
        assert_eq!(byte_offset_at_width("日本語", 5), 6);
        assert_eq!(byte_offset_at_width("日本語", 1), 0);
    }

    #[test]
    fn truncate_borrows_when_it_fits() {
        assert!(matches!(truncate_to_width("abc", 3), Cow::Borrowed("abc")));
        assert_eq!(truncate_to_width("abcdef", 4), "abc…");
    }
}
