//! Line wrapping for text layout.
//!
//! This module provides text wrapping functionality with proper word
//! boundary detection for multiple scripts including:
//! - ASCII alphanumeric
//! - Latin Extended (French, German, Spanish, etc.)
//! - Cyrillic (Russian, Ukrainian, etc.)
//! - Vietnamese
//! - CJK (Chinese, Japanese, Korean) - breaks at any character

use std::collections::HashMap;
use std::iter;
use std::sync::Arc;

use super::FontId;

/// Determines whether to truncate text from the start or end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TruncateFrom {
    /// Truncate text from the start (show end of text).
    Start,
    /// Truncate text from the end (show start of text).
    End,
}

/// Text wrapping engine.
///
/// Wraps text to a given width using cached character widths for performance.
pub struct LineWrapper {
    font_id: FontId,
    font_size: f32,
    /// Cached widths for ASCII characters (fast path).
    cached_ascii_char_widths: [Option<f32>; 128],
    /// Cached widths for non-ASCII characters.
    cached_other_char_widths: HashMap<char, f32>,
    /// Character width lookup function.
    char_width_fn: Arc<dyn Fn(char) -> f32 + Send + Sync>,
}

impl LineWrapper {
    /// Maximum indent that can be applied to a wrapped line.
    pub const MAX_INDENT: u32 = 256;

    /// Create a new line wrapper.
    ///
    /// The `char_width_fn` should return the width of a character in pixels.
    pub fn new(
        font_id: FontId,
        font_size: f32,
        char_width_fn: impl Fn(char) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self {
            font_id,
            font_size,
            cached_ascii_char_widths: [None; 128],
            cached_other_char_widths: HashMap::default(),
            char_width_fn: Arc::new(char_width_fn),
        }
    }

    /// Create a line wrapper with a fixed character width (for monospace fonts).
    pub fn new_monospace(font_id: FontId, font_size: f32, char_width: f32) -> Self {
        Self::new(font_id, font_size, move |_| char_width)
    }

    /// Get the font ID for this wrapper.
    pub fn font_id(&self) -> FontId {
        self.font_id
    }

    /// Get the font size for this wrapper.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Wrap a line of text to the given width.
    ///
    /// Returns an iterator of wrap boundaries.
    pub fn wrap_line<'a>(
        &'a mut self,
        fragments: &'a [LineFragment<'a>],
        wrap_width: f32,
    ) -> impl Iterator<Item = Boundary> + 'a {
        let mut width = 0.0;
        let mut first_non_whitespace_ix = None;
        let mut indent = None;
        let mut last_candidate_ix = 0;
        let mut last_candidate_width = 0.0;
        let mut last_wrap_ix = 0;
        let mut prev_c = '\0';
        let mut index = 0;
        let mut candidates = fragments
            .iter()
            .flat_map(move |fragment| fragment.wrap_boundary_candidates())
            .peekable();

        iter::from_fn(move || {
            for candidate in candidates.by_ref() {
                let ix = index;
                index += candidate.len_utf8();
                let mut new_prev_c = prev_c;

                let item_width = match candidate {
                    WrapBoundaryCandidate::Char { character: c } => {
                        if c == '\n' {
                            continue;
                        }

                        if Self::is_word_char(c) {
                            if prev_c == ' ' && c != ' ' && first_non_whitespace_ix.is_some() {
                                last_candidate_ix = ix;
                                last_candidate_width = width;
                            }
                        } else {
                            // CJK may not be space separated
                            if c != ' ' && first_non_whitespace_ix.is_some() {
                                last_candidate_ix = ix;
                                last_candidate_width = width;
                            }
                        }

                        if c != ' ' && first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        new_prev_c = c;
                        self.width_for_char(c)
                    }
                    WrapBoundaryCandidate::Element {
                        width: element_width,
                        ..
                    } => {
                        if prev_c == ' ' && first_non_whitespace_ix.is_some() {
                            last_candidate_ix = ix;
                            last_candidate_width = width;
                        }

                        if first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        element_width
                    }
                };

                width += item_width;
                if width > wrap_width && ix > last_wrap_ix {
                    if let (None, Some(first_non_whitespace)) = (indent, first_non_whitespace_ix) {
                        indent = Some(
                            Self::MAX_INDENT.min((first_non_whitespace - last_wrap_ix) as u32),
                        );
                    }

                    if last_candidate_ix > 0 {
                        last_wrap_ix = last_candidate_ix;
                        width -= last_candidate_width;
                        last_candidate_ix = 0;
                    } else {
                        last_wrap_ix = ix;
                        width = item_width;
                    }

                    if let Some(indent_spaces) = indent {
                        width += self.width_for_char(' ') * indent_spaces as f32;
                    }

                    return Some(Boundary::new(last_wrap_ix, indent.unwrap_or(0)));
                }

                prev_c = new_prev_c;
            }

            None
        })
    }

    /// Determine if a line should be truncated.
    ///
    /// Returns the truncation index if truncation is needed.
    pub fn should_truncate_line(
        &mut self,
        line: &str,
        truncate_width: f32,
        truncation_affix: &str,
        truncate_from: TruncateFrom,
    ) -> Option<usize> {
        let mut width = 0.0;
        let suffix_width: f32 = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .sum();
        let mut truncate_ix = 0;

        match truncate_from {
            TruncateFrom::Start => {
                for (ix, c) in line.char_indices().rev() {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    let char_width = self.width_for_char(c);
                    width += char_width;

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
            TruncateFrom::End => {
                for (ix, c) in line.char_indices() {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    let char_width = self.width_for_char(c);
                    width += char_width;

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
        }

        None
    }

    /// Truncate a line of text to the given width.
    ///
    /// Returns the truncated text with the truncation affix applied.
    pub fn truncate_line(
        &mut self,
        line: &str,
        truncate_width: f32,
        truncation_affix: &str,
        truncate_from: TruncateFrom,
    ) -> String {
        if let Some(truncate_ix) =
            self.should_truncate_line(line, truncate_width, truncation_affix, truncate_from)
        {
            match truncate_from {
                TruncateFrom::Start => {
                    format!("{}{}", truncation_affix, &line[truncate_ix + 1..])
                }
                TruncateFrom::End => {
                    format!("{}{}", &line[..truncate_ix], truncation_affix)
                }
            }
        } else {
            line.to_string()
        }
    }

    /// Determine if a character should be treated as part of a word.
    ///
    /// Word characters should not be broken across lines.
    pub fn is_word_char(c: char) -> bool {
        // ASCII alphanumeric
        c.is_ascii_alphanumeric() ||
        // Latin-1 Supplement (French, German, Spanish accented chars)
        matches!(c, '\u{00C0}'..='\u{00FF}') ||
        // Latin Extended-A
        matches!(c, '\u{0100}'..='\u{017F}') ||
        // Latin Extended-B
        matches!(c, '\u{0180}'..='\u{024F}') ||
        // Cyrillic (Russian, Ukrainian, etc.)
        matches!(c, '\u{0400}'..='\u{04FF}') ||
        // Vietnamese - Latin Extended Additional
        matches!(c, '\u{1E00}'..='\u{1EFF}') ||
        // Combining Diacritical Marks
        matches!(c, '\u{0300}'..='\u{036F}') ||
        // Common punctuation that should stay with words
        matches!(c, '-' | '_' | '.' | '\'' | '$' | '%' | '@' | '#' | '^' | '~' | ',' | '=' | ':') ||
        // Special ellipsis character (keep at end of line)
        matches!(c, '⋯')
    }

    /// Get the width of a character, using cache when available.
    #[inline(always)]
    pub fn width_for_char(&mut self, c: char) -> f32 {
        if (c as u32) < 128 {
            if let Some(cached_width) = self.cached_ascii_char_widths[c as usize] {
                cached_width
            } else {
                let width = (self.char_width_fn)(c);
                self.cached_ascii_char_widths[c as usize] = Some(width);
                width
            }
        } else if let Some(cached_width) = self.cached_other_char_widths.get(&c) {
            *cached_width
        } else {
            let width = (self.char_width_fn)(c);
            self.cached_other_char_widths.insert(c, width);
            width
        }
    }

    /// Clear the character width cache.
    pub fn clear_cache(&mut self) {
        self.cached_ascii_char_widths = [None; 128];
        self.cached_other_char_widths.clear();
    }
}

/// A fragment of a line that can be wrapped.
#[derive(Debug, Clone)]
pub enum LineFragment<'a> {
    /// A text fragment consisting of characters.
    Text {
        /// The text content.
        text: &'a str,
    },
    /// A non-text element with a fixed width.
    Element {
        /// The width of the element in pixels.
        width: f32,
        /// The UTF-8 encoded length of the element.
        len_utf8: usize,
    },
}

impl<'a> LineFragment<'a> {
    /// Create a text fragment.
    pub fn text(text: &'a str) -> Self {
        LineFragment::Text { text }
    }

    /// Create a non-text element fragment.
    pub fn element(width: f32, len_utf8: usize) -> Self {
        LineFragment::Element { width, len_utf8 }
    }

    fn wrap_boundary_candidates(&self) -> impl Iterator<Item = WrapBoundaryCandidate> + '_ {
        let text = match self {
            LineFragment::Text { text } => *text,
            LineFragment::Element { .. } => "\0",
        };
        text.chars().map(move |character| {
            if let LineFragment::Element { width, len_utf8 } = self {
                WrapBoundaryCandidate::Element {
                    width: *width,
                    len_utf8: *len_utf8,
                }
            } else {
                WrapBoundaryCandidate::Char { character }
            }
        })
    }
}

#[derive(Debug, Clone)]
enum WrapBoundaryCandidate {
    Char { character: char },
    Element { width: f32, len_utf8: usize },
}

impl WrapBoundaryCandidate {
    fn len_utf8(&self) -> usize {
        match self {
            WrapBoundaryCandidate::Char { character } => character.len_utf8(),
            WrapBoundaryCandidate::Element { len_utf8, .. } => *len_utf8,
        }
    }
}

/// A boundary where a line wraps.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The byte index where the line wraps.
    pub ix: usize,
    /// The indent (in spaces) for the next line.
    pub next_indent: u32,
}

impl Boundary {
    /// Create a new boundary.
    pub fn new(ix: usize, next_indent: u32) -> Self {
        Self { ix, next_indent }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wrapper() -> LineWrapper {
        // Use a fixed 10px per character for testing
        LineWrapper::new_monospace(0, 16.0, 10.0)
    }

    #[test]
    fn test_wrap_line_basic() {
        let mut wrapper = make_wrapper();

        // "aa bbb cccc" at 10px per char with 35px width
        // Total: 11 chars = 110px, should wrap multiple times
        let boundaries: Vec<_> = wrapper
            .wrap_line(&[LineFragment::text("aa bbb cccc")], 35.0)
            .collect();

        // Should wrap after "aa " (30px) and after "bbb " (40px from wrap point)
        assert!(!boundaries.is_empty());
    }

    #[test]
    fn test_wrap_line_with_cjk() {
        let mut wrapper = make_wrapper();

        // CJK characters can break at any point
        let boundaries: Vec<_> = wrapper
            .wrap_line(&[LineFragment::text("你好世界")], 25.0)
            .collect();

        // Should have some wrap boundaries
        assert!(!boundaries.is_empty());
    }

    #[test]
    fn test_is_word_char() {
        // ASCII
        assert!(LineWrapper::is_word_char('a'));
        assert!(LineWrapper::is_word_char('Z'));
        assert!(LineWrapper::is_word_char('5'));

        // Latin Extended
        assert!(LineWrapper::is_word_char('é'));
        assert!(LineWrapper::is_word_char('ñ'));
        assert!(LineWrapper::is_word_char('ü'));

        // Cyrillic
        assert!(LineWrapper::is_word_char('д'));
        assert!(LineWrapper::is_word_char('Я'));

        // Punctuation that stays with words
        assert!(LineWrapper::is_word_char('-'));
        assert!(LineWrapper::is_word_char('_'));
        assert!(LineWrapper::is_word_char('.'));

        // NOT word chars
        assert!(!LineWrapper::is_word_char(' '));
        assert!(!LineWrapper::is_word_char('你')); // CJK can break
        assert!(!LineWrapper::is_word_char('/'));
        assert!(!LineWrapper::is_word_char('('));
    }

    #[test]
    fn test_truncate_line_end() {
        let mut wrapper = make_wrapper();

        let result = wrapper.truncate_line("Hello World", 55.0, "...", TruncateFrom::End);
        assert!(result.ends_with("..."));
        assert!(result.len() < "Hello World".len() + 3);
    }

    #[test]
    fn test_truncate_line_start() {
        let mut wrapper = make_wrapper();

        let result = wrapper.truncate_line("Hello World", 55.0, "...", TruncateFrom::Start);
        assert!(result.starts_with("..."));
    }

    #[test]
    fn test_width_caching() {
        let mut wrapper = make_wrapper();

        // First call computes
        let w1 = wrapper.width_for_char('a');
        // Second call should use cache
        let w2 = wrapper.width_for_char('a');
        assert_eq!(w1, w2);

        // Non-ASCII
        let w3 = wrapper.width_for_char('é');
        let w4 = wrapper.width_for_char('é');
        assert_eq!(w3, w4);
    }

    #[test]
    fn test_line_fragment_element() {
        let mut wrapper = make_wrapper();

        // Mix of text and fixed-width element
        let boundaries: Vec<_> = wrapper
            .wrap_line(
                &[
                    LineFragment::text("Hello "),
                    LineFragment::element(50.0, 1),
                    LineFragment::text(" World"),
                ],
                60.0,
            )
            .collect();

        // Should wrap around the element
        assert!(!boundaries.is_empty());
    }

    #[test]
    fn test_boundary() {
        let b = Boundary::new(10, 4);
        assert_eq!(b.ix, 10);
        assert_eq!(b.next_indent, 4);
    }
}
