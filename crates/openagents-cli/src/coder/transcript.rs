//! Streaming markdown content for one transcript entry.
//!
//! Wraps [`StreamingMarkdownRenderer`] with the two caches a TUI needs:
//!
//! 1. The **engine's** checkpoint freeze — only the tail after the last stable
//!    block boundary is reparsed as tokens arrive.
//! 2. A **wrap cache** keyed by `(width, generation)` — frozen pre-wrap lines
//!    are word-wrapped once and kept; only the tail is re-wrapped per chunk,
//!    and a resize re-wraps everything exactly once.
//!
//! Together these turn streaming from `O(n²)` into roughly `O(n)`. Both are
//! observable through [`WrapStats`] and
//! [`StreamingMarkdownRenderer::reparsed_bytes`] so the tests can assert the
//! saving rather than assume it.
//!
//! The wrap-cache shape follows grok-build's
//! `xai-grok-pager/src/scrollback/blocks/markdown_content.rs` (Apache-2.0; see
//! `src/markdown/LICENSE-APACHE-xai`).

use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

use crate::coder::markdown::streaming::StreamingMarkdownRenderer;
use crate::coder::markdown::theme;
use crate::coder::markdown::wrapping::word_wrap_lines_with_joiners;

/// A hyperlink positioned on a *wrapped* row, ready for OSC 8 emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenLink {
    /// Row index into the wrapped lines of this content block.
    pub row: usize,
    /// First display column of the link text on that row.
    pub col_start: usize,
    /// One past the last display column.
    pub col_end: usize,
    /// Destination URL.
    pub url: String,
    /// Shared across the fragments of one logical link, so a terminal can
    /// hover-group a link that wrapped across rows.
    pub id: u32,
}

/// Work actually performed by the wrap cache, for tests and diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WrapStats {
    /// Pre-wrap lines handed to the word wrapper since construction.
    ///
    /// With freezing this grows by roughly the size of the open tail per
    /// chunk. Without freezing it would grow by the whole document per chunk.
    pub lines_wrapped: usize,
    /// Number of `ensure_wrapped` calls that did real work.
    pub wrap_passes: usize,
    /// Number of `ensure_wrapped` calls served entirely from cache.
    pub cache_hits: usize,
    /// Number of full re-wraps caused by a width change.
    pub width_rewraps: usize,
}

/// Streaming markdown for one assistant message.
#[derive(Debug)]
pub struct MarkdownContent {
    renderer: StreamingMarkdownRenderer,
    /// Bumped whenever the source changes, so the wrap cache knows to refresh.
    generation: u64,

    cache_width: usize,
    cache_generation: u64,
    cache_lines: Vec<Line<'static>>,
    cache_joiners: Vec<Option<String>>,
    cache_links: Vec<ScreenLink>,

    /// How many *pre-wrap* lines have already been folded into the frozen
    /// portion of the cache.
    frozen_pre_wrap_count: usize,
    /// How many *wrapped* rows those frozen pre-wrap lines produced.
    frozen_wrapped_count: usize,

    stats: WrapStats,
}

impl Clone for MarkdownContent {
    fn clone(&self) -> Self {
        // Rebuild from source rather than copying caches: a clone that shares
        // a stale frozen count would render a different document.
        let mut next = Self::new();
        next.push(self.renderer.source());
        next
    }
}

impl Default for MarkdownContent {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownContent {
    /// A renderer using Coder's amber palette, in pretty mode.
    pub fn new() -> Self {
        Self {
            renderer: StreamingMarkdownRenderer::new(theme::coder_markdown_style(), true),
            generation: 0,
            cache_width: 0,
            cache_generation: u64::MAX,
            cache_lines: Vec::new(),
            cache_joiners: Vec::new(),
            cache_links: Vec::new(),
            frozen_pre_wrap_count: 0,
            frozen_wrapped_count: 0,
            stats: WrapStats::default(),
        }
    }

    /// Append a streamed chunk.
    ///
    /// Parsing is deferred to the next [`Self::lines`] call so a chunk is
    /// parsed once, at the width it will actually be drawn at, rather than
    /// once here and again on resize. Only the unfrozen tail is reparsed.
    pub fn push(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.renderer.push(chunk);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Flush the LaTeX normalizer's held-back bytes and run the final scan.
    ///
    /// Call this when the model stops streaming. Until it is called, a chunk
    /// boundary that split `\(` in half is held back rather than rendered as a
    /// stray backslash.
    pub fn finish(&mut self) {
        self.renderer.finish(Some(theme::syntect()));
        self.generation = self.generation.wrapping_add(1);
    }

    /// The accumulated markdown source, after LaTeX delimiter normalization.
    pub fn source(&self) -> &str {
        self.renderer.source()
    }

    /// Pre-wrap lines the engine currently considers frozen.
    pub fn frozen_lines_count(&self) -> usize {
        self.renderer.frozen_lines_count()
    }

    /// Total source bytes reparsed across this content's lifetime.
    pub fn reparsed_bytes(&self) -> u64 {
        self.renderer.reparsed_bytes()
    }

    /// Work the wrap cache actually did.
    pub fn stats(&self) -> WrapStats {
        self.stats
    }

    /// Pre-wrap lines as the engine rendered them, before word wrapping.
    pub fn pre_wrap_lines(&self) -> &[Line<'static>] {
        self.renderer.view().lines
    }

    /// Wrapped, amber-flattened lines for `width` columns.
    pub fn lines(&mut self, width: usize) -> &[Line<'static>] {
        self.ensure_wrapped(width);
        &self.cache_lines
    }

    /// Hyperlinks positioned on the wrapped rows for `width` columns.
    pub fn links(&mut self, width: usize) -> &[ScreenLink] {
        self.ensure_wrapped(width);
        &self.cache_links
    }

    /// Soft-wrap joiners parallel to [`Self::lines`].
    ///
    /// Row `i` has `Some(joiner)` when it continues row `i - 1`; the joiner is
    /// the exact text the wrapper skipped at the break, so joining
    /// `rows[i-1] + joiner + rows[i]` reconstructs the unwrapped line. Row `0`
    /// of each pre-wrap line has `None`.
    pub fn joiners(&mut self, width: usize) -> &[Option<String>] {
        self.ensure_wrapped(width);
        &self.cache_joiners
    }

    /// Reconstruct the unwrapped text of the wrapped rows, using the joiners.
    ///
    /// This is what "copy" should yield: the wrap points vanish and the
    /// skipped whitespace comes back.
    pub fn unwrapped_text(&mut self, width: usize) -> String {
        self.ensure_wrapped(width);
        let mut out = String::new();
        for (i, line) in self.cache_lines.iter().enumerate() {
            match self.cache_joiners.get(i).and_then(|j| j.as_deref()) {
                Some(joiner) => out.push_str(joiner),
                None if i > 0 => out.push('\n'),
                None => {}
            }
            for span in &line.spans {
                out.push_str(span.content.as_ref());
            }
        }
        out
    }

    /// Populate the wrap cache for `width`, re-wrapping only what changed.
    fn ensure_wrapped(&mut self, width: usize) {
        let width = width.max(1);

        if self.cache_width == width && self.cache_generation == self.generation {
            self.stats.cache_hits += 1;
            return;
        }
        self.stats.wrap_passes += 1;

        // A width change invalidates every wrapped row, frozen or not.
        if self.cache_width != width {
            if self.cache_width != 0 {
                self.stats.width_rewraps += 1;
            }
            self.frozen_pre_wrap_count = 0;
            self.frozen_wrapped_count = 0;
            self.cache_lines.clear();
            self.cache_joiners.clear();
        }

        self.renderer.set_max_table_width(Some(width));
        // `set_max_table_width` drops the engine's frozen state when the width
        // actually changed, so re-render before reading the frozen count.
        self.renderer.render(Some(theme::syntect()));

        let frozen_count = self.renderer.frozen_lines_count();
        let total_lines = self.renderer.view().lines.len();

        // Newly frozen pre-wrap lines: wrapped once, then never again.
        let new_frozen = if frozen_count > self.frozen_pre_wrap_count {
            let slice =
                self.renderer.view().lines[self.frozen_pre_wrap_count..frozen_count].to_vec();
            self.stats.lines_wrapped += slice.len();
            Some(word_wrap_lines_with_joiners(slice, width))
        } else {
            None
        };

        // The open tail: re-wrapped on every chunk, but it is bounded by the
        // distance back to the last checkpoint, not by document length.
        let tail = if frozen_count < total_lines {
            let slice = self.renderer.view().lines[frozen_count..].to_vec();
            self.stats.lines_wrapped += slice.len();
            Some(word_wrap_lines_with_joiners(slice, width))
        } else {
            None
        };

        // Pre-wrap lines are needed for hyperlink column mapping; take them
        // before mutating the cache.
        let pre_wrap: Vec<Line<'static>> = self.renderer.view().lines.to_vec();
        let hyperlinks = self.renderer.view().hyperlinks.to_vec();

        let keep = self.frozen_wrapped_count;
        self.cache_lines.truncate(keep);
        self.cache_joiners.truncate(keep);

        if let Some((lines, joiners)) = new_frozen {
            self.cache_lines.extend(lines);
            self.cache_joiners.extend(joiners);
            self.frozen_pre_wrap_count = frozen_count;
            self.frozen_wrapped_count = self.cache_lines.len();
        }
        if let Some((lines, joiners)) = tail {
            self.cache_lines.extend(lines);
            self.cache_joiners.extend(joiners);
        }

        // Flatten to Coder's amber. Done on the whole cache each pass
        // because a frozen row re-enters the cache untouched; it is a cheap
        // per-span style write with no allocation.
        theme::amberize(&mut self.cache_lines);

        self.cache_links = map_links_to_rows(
            &pre_wrap,
            &self.cache_lines,
            &self.cache_joiners,
            &hyperlinks,
        );

        self.cache_width = width;
        self.cache_generation = self.generation;
    }
}

/// Map hyperlinks from pre-wrap coordinates onto wrapped rows.
///
/// A row whose joiner is `None` starts a new pre-wrap line; a row with
/// `Some(joiner)` continues the previous one, having skipped `joiner` at the
/// break. Walking that structure recovers, for each row, which pre-wrap line
/// it came from and how many display columns of that line precede it — which
/// is exactly what turns a pre-wrap column range into a screen column range.
///
/// Links whose pre-wrap line cannot be located are dropped rather than guessed
/// at: a hyperlink painted over the wrong text is worse than no hyperlink.
fn map_links_to_rows(
    pre_wrap: &[Line<'static>],
    wrapped: &[Line<'static>],
    joiners: &[Option<String>],
    hyperlinks: &[crate::coder::markdown::HyperlinkTarget],
) -> Vec<ScreenLink> {
    if hyperlinks.is_empty() {
        return Vec::new();
    }

    // For each wrapped row: (pre-wrap line index, columns of that line already
    // consumed, injected-indent width on this row, content width on this row).
    let mut rows: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(wrapped.len());
    let mut pre_idx = 0usize;
    let mut consumed = 0usize;

    for (row, line) in wrapped.iter().enumerate() {
        let is_continuation = joiners.get(row).map(|j| j.is_some()).unwrap_or(false);
        if !is_continuation {
            if row > 0 {
                pre_idx += 1;
            }
            consumed = 0;
        }

        // Continuation rows of a blockquote carry a re-injected `│ ` prefix
        // that is not part of the pre-wrap line's own columns.
        let indent = if is_continuation {
            pre_wrap
                .get(pre_idx)
                .map(|l| {
                    let flat: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                    let bytes = crate::coder::markdown::wrapping::blockquote_prefix_len(&flat);
                    flat[..bytes].width()
                })
                .unwrap_or(0)
        } else {
            0
        };

        let total = line_width(line);
        let content = total.saturating_sub(indent);
        rows.push((pre_idx, consumed, indent, content));

        let joiner_width = joiners
            .get(row)
            .and_then(|j| j.as_deref())
            .map(UnicodeWidthStr::width)
            .unwrap_or(0);
        consumed += content + joiner_width;
    }

    let mut out = Vec::new();
    for link in hyperlinks {
        for (row, &(pre, start_col, indent, content)) in rows.iter().enumerate() {
            if pre != link.line_index || content == 0 {
                continue;
            }
            let row_end = start_col + content;
            let lo = link.column_range.start.max(start_col);
            let hi = link.column_range.end.min(row_end);
            if lo >= hi {
                continue;
            }
            out.push(ScreenLink {
                row,
                col_start: indent + (lo - start_col),
                col_end: indent + (hi - start_col),
                url: link.url.clone(),
                id: link.id,
            });
        }
    }
    out
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wrap_cache_serves_repeat_calls_at_the_same_width() {
        let mut md = MarkdownContent::new();
        md.push("hello world\n\nmore text\n\n");
        let _ = md.lines(40);
        let before = md.stats();
        let _ = md.lines(40);
        let _ = md.lines(40);
        let after = md.stats();
        assert_eq!(after.wrap_passes, before.wrap_passes);
        assert_eq!(after.cache_hits, before.cache_hits + 2);
    }

    #[test]
    fn width_change_forces_exactly_one_rewrap() {
        let mut md = MarkdownContent::new();
        md.push("a fairly long paragraph that will definitely wrap at narrow widths\n\n");
        let _ = md.lines(80);
        let baseline = md.stats().width_rewraps;
        let _ = md.lines(30);
        let _ = md.lines(30);
        assert_eq!(md.stats().width_rewraps, baseline + 1);
    }

    #[test]
    fn joiners_reconstruct_the_unwrapped_paragraph() {
        let mut md = MarkdownContent::new();
        md.push("alpha beta gamma delta epsilon zeta eta theta\n\n");
        md.finish();
        let wide = {
            let mut w = MarkdownContent::new();
            w.push("alpha beta gamma delta epsilon zeta eta theta\n\n");
            w.finish();
            w.unwrapped_text(200)
        };
        let narrow = md.unwrapped_text(12);
        assert!(md.lines(12).len() > 1, "expected the paragraph to wrap");
        assert_eq!(narrow.trim_end(), wide.trim_end());
    }

    #[test]
    fn cjk_wraps_on_display_columns_not_char_counts() {
        let mut md = MarkdownContent::new();
        // Six ideographs = 12 display columns.
        md.push("日本語日本語\n\n");
        md.finish();
        let lines = text_of(md.lines(6));
        let content: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
        assert!(
            content.iter().all(|l| l.width() <= 6),
            "a row exceeded 6 display columns: {content:?}"
        );
        assert!(content.len() >= 2, "expected a wrap: {content:?}");
    }

    #[test]
    fn link_columns_land_on_the_link_text() {
        let mut md = MarkdownContent::new();
        md.push("see [Buildkite](https://buildkite.com/) now\n\n");
        md.finish();
        let lines = text_of(md.lines(80));
        let links = md.links(80).to_vec();
        assert!(!links.is_empty(), "expected at least one hyperlink");

        let painted: Vec<String> = links
            .iter()
            .map(|target| {
                let row = &lines[target.row];
                let mut col = 0usize;
                let mut out = String::new();
                for ch in row.chars() {
                    let w = UnicodeWidthStr::width(ch.to_string().as_str());
                    if col >= target.col_start && col < target.col_end {
                        out.push(ch);
                    }
                    col += w;
                }
                out
            })
            .collect();

        assert!(
            painted.iter().any(|p| p == "Buildkite"),
            "no hyperlink covered exactly the link text; painted={painted:?} lines={lines:?}"
        );
        assert!(
            links.iter().all(|l| l.url == "https://buildkite.com/"),
            "a hyperlink pointed somewhere the source never mentioned: {links:?}"
        );
    }
}
