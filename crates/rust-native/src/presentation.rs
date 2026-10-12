//! Text presentation for symbols that can draw as emoji (#11476).
//!
//! Unicode marks some symbols, such as `ℹ`, `✔`, `✖`, and `⚠`, as emoji
//! whose default presentation is text. A platform text engine still draws
//! them from a color emoji face when the text face lacks the glyph, so a test
//! runner's `ℹ tests 12` shows a blue emoji on the iPhone. The transcript
//! layout appends the text variation selector, U+FE0E, after each such symbol
//! that has no selector of its own, so every surface draws it as text.

use std::borrow::Cow;

/// The text variation selector.
pub const TEXT_SELECTOR: char = '\u{fe0e}';
const EMOJI_SELECTOR: char = '\u{fe0f}';

/// Symbols in the Basic Multilingual Plane whose Unicode properties are
/// `Emoji=Yes` and `Emoji_Presentation=No`: text by default, emoji with
/// U+FE0F or a missing glyph. Digits, `#`, and `*` are left out; every text
/// face has them. Inclusive ranges, ascending.
const TEXT_DEFAULT: &[(u32, u32)] = &[
    (0x00A9, 0x00A9),
    (0x00AE, 0x00AE),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x2122, 0x2122),
    (0x2139, 0x2139),
    (0x2194, 0x2199),
    (0x21A9, 0x21AA),
    (0x2328, 0x2328),
    (0x23CF, 0x23CF),
    (0x23ED, 0x23EF),
    (0x23F1, 0x23F2),
    (0x23F8, 0x23FA),
    (0x24C2, 0x24C2),
    (0x25AA, 0x25AB),
    (0x25B6, 0x25B6),
    (0x25C0, 0x25C0),
    (0x25FB, 0x25FC),
    (0x2600, 0x2604),
    (0x260E, 0x260E),
    (0x2611, 0x2611),
    (0x2618, 0x2618),
    (0x261D, 0x261D),
    (0x2620, 0x2620),
    (0x2622, 0x2623),
    (0x2626, 0x2626),
    (0x262A, 0x262A),
    (0x262E, 0x262F),
    (0x2638, 0x263A),
    (0x2640, 0x2640),
    (0x2642, 0x2642),
    (0x265F, 0x2660),
    (0x2663, 0x2663),
    (0x2665, 0x2666),
    (0x2668, 0x2668),
    (0x267B, 0x267B),
    (0x267E, 0x267E),
    (0x2692, 0x2692),
    (0x2694, 0x2697),
    (0x2699, 0x2699),
    (0x269B, 0x269C),
    (0x26A0, 0x26A0),
    (0x26A7, 0x26A7),
    (0x26B0, 0x26B1),
    (0x26C8, 0x26C8),
    (0x26CF, 0x26CF),
    (0x26D1, 0x26D1),
    (0x26D3, 0x26D3),
    (0x26E9, 0x26E9),
    (0x26F0, 0x26F1),
    (0x26F4, 0x26F4),
    (0x26F7, 0x26F9),
    (0x2702, 0x2702),
    (0x2708, 0x2709),
    (0x270C, 0x270D),
    (0x270F, 0x270F),
    (0x2712, 0x2712),
    (0x2714, 0x2714),
    (0x2716, 0x2716),
    (0x271D, 0x271D),
    (0x2721, 0x2721),
    (0x2733, 0x2734),
    (0x2744, 0x2744),
    (0x2747, 0x2747),
    (0x2763, 0x2764),
    (0x27A1, 0x27A1),
    (0x2934, 0x2935),
    (0x2B05, 0x2B07),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0x3297, 0x3297),
    (0x3299, 0x3299),
];

/// Whether `ch` is a symbol that is text by default but can draw as emoji.
#[must_use]
pub fn is_text_default_emoji(ch: char) -> bool {
    let code = u32::from(ch);
    let at = TEXT_DEFAULT.partition_point(|(start, _)| *start <= code);
    at.checked_sub(1)
        .is_some_and(|index| code <= TEXT_DEFAULT[index].1)
}

/// Whether a selector after the symbol would change or break what follows:
/// a variation selector already chose, or a keycap, joiner, or skin-tone
/// modifier continues an emoji sequence.
fn continues(next: Option<char>) -> bool {
    matches!(
        next,
        Some(TEXT_SELECTOR | EMOJI_SELECTOR | '\u{20e3}' | '\u{200d}' | '\u{1f3fb}'..='\u{1f3ff}')
    )
}

/// `text` with U+FE0E after each text-default emoji symbol that has no
/// selector or sequence of its own, so it draws as text. Borrowed when
/// nothing changes.
#[must_use]
pub fn text_presentation(text: &str) -> Cow<'_, str> {
    let mut chars = text.char_indices().peekable();
    let mut out: Option<String> = None;
    while let Some((at, ch)) = chars.next() {
        let next = chars.peek().map(|(_, next)| *next);
        let add = is_text_default_emoji(ch) && !continues(next);
        if add && out.is_none() {
            let mut started = String::with_capacity(text.len() + 6);
            started.push_str(&text[..at]);
            out = Some(started);
        }
        if let Some(out) = out.as_mut() {
            out.push(ch);
            if add {
                out.push(TEXT_SELECTOR);
            }
        }
    }
    out.map_or(Cow::Borrowed(text), Cow::Owned)
}

/// The inverse of [`text_presentation`] for copying: drops U+FE0E after a
/// text-default emoji symbol, which draws the same without it.
#[must_use]
pub fn without_text_presentation(text: &str) -> Cow<'_, str> {
    if !text.contains(TEXT_SELECTOR) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut previous: Option<char> = None;
    for ch in text.chars() {
        if ch == TEXT_SELECTOR && previous.is_some_and(is_text_default_emoji) {
            previous = Some(ch);
            continue;
        }
        out.push(ch);
        previous = Some(ch);
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_marks_draw_as_text() {
        let summary = "ℹ tests 12\nℹ pass 12\n✔ adds\n✖ fails\n⚠ slow";
        assert_eq!(
            text_presentation(summary),
            "ℹ\u{fe0e} tests 12\nℹ\u{fe0e} pass 12\n✔\u{fe0e} adds\n✖\u{fe0e} fails\n⚠\u{fe0e} slow"
        );
    }

    #[test]
    fn plain_text_and_chosen_presentations_are_kept() {
        assert!(matches!(text_presentation("12 passed"), Cow::Borrowed(_)));
        assert!(matches!(text_presentation("🚀 ✅ done"), Cow::Borrowed(_)));
        // The author chose: emoji, text, or an emoji sequence.
        assert_eq!(text_presentation("❤\u{fe0f}"), "❤\u{fe0f}");
        assert_eq!(text_presentation("ℹ\u{fe0e}"), "ℹ\u{fe0e}");
        assert_eq!(
            text_presentation("❤\u{200d}🔥 ☝\u{1f3fd}"),
            "❤\u{200d}🔥 ☝\u{1f3fd}"
        );
        // A symbol at the end still gets its selector.
        assert_eq!(text_presentation("done ✔"), "done ✔\u{fe0e}");
    }

    #[test]
    fn copying_drops_the_added_selectors() {
        let summary = "ℹ tests 12 ✔ ok";
        assert_eq!(
            without_text_presentation(&text_presentation(summary)),
            summary
        );
        assert_eq!(without_text_presentation("a\u{fe0e}"), "a\u{fe0e}");
    }
}
