//! A bounded transcript with a wrap cache.
//!
//! A long session pushes thousands of lines, and a frame that re-wraps
//! every one of them at every redraw spends its time on text nobody sees.
//! [`Scrollback`] keeps at most [`LINES_MAX`] lines, evicting the oldest,
//! and wraps each line once per width: a line's rows are computed when the
//! frame first asks for them, kept until the width changes, and thrown
//! away for every line at once when it does. A frame therefore wraps the
//! lines pushed since the last frame, or all of them after a resize, and
//! none otherwise.
//!
//! The line and row types are the shell's own; the cache holds whatever the
//! wrap function returns.

use std::collections::VecDeque;

/// The most lines a scrollback keeps. Older lines are gone from the screen
/// but not from the session's trace, which records the whole conversation.
pub const LINES_MAX: usize = 5_000;

struct Entry<L, R> {
    line: L,
    rows: Option<Vec<R>>,
}

/// A bounded transcript whose lines wrap once per width.
pub struct Scrollback<L, R, W>
where
    W: Fn(&L, usize) -> Vec<R>,
{
    entries: VecDeque<Entry<L, R>>,
    limit: usize,
    width: usize,
    wrap: W,
    evicted: usize,
}

impl<L, R, W> Scrollback<L, R, W>
where
    W: Fn(&L, usize) -> Vec<R>,
{
    /// An empty scrollback holding at most [`LINES_MAX`] lines, wrapping
    /// each with `wrap` when a frame asks.
    pub fn new(wrap: W) -> Self {
        Self::with_limit(LINES_MAX, wrap)
    }

    /// An empty scrollback holding at most `limit` lines.
    pub fn with_limit(limit: usize, wrap: W) -> Self {
        Self {
            entries: VecDeque::new(),
            limit: limit.max(1),
            width: 0,
            wrap,
            evicted: 0,
        }
    }

    /// Appends `line`, evicting the oldest line when the scrollback is full.
    pub fn push(&mut self, line: L) {
        if self.entries.len() == self.limit {
            self.entries.pop_front();
            self.evicted += 1;
        }
        self.entries.push_back(Entry { line, rows: None });
    }

    /// How many lines are kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is kept.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many lines the limit has pushed out since the session began.
    pub fn evicted(&self) -> usize {
        self.evicted
    }

    /// The lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &L> {
        self.entries.iter().map(|entry| &entry.line)
    }

    /// The rows of every line `keep` admits, oldest first, wrapped at
    /// `width`. A width other than the last one wraps every line afresh;
    /// otherwise only lines with no rows yet are wrapped.
    pub fn rows(&mut self, width: usize, keep: impl Fn(&L) -> bool) -> Vec<&R> {
        if width != self.width {
            self.width = width;
            for entry in &mut self.entries {
                entry.rows = None;
            }
        }
        let wrap = &self.wrap;
        for entry in &mut self.entries {
            if entry.rows.is_none() && keep(&entry.line) {
                entry.rows = Some(wrap(&entry.line, width));
            }
        }
        self.entries
            .iter()
            .filter(|entry| keep(&entry.line))
            .flat_map(|entry| entry.rows.as_deref().unwrap_or_default())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::wrap_rows;

    /// Wraps plain text into one `String` per row, counting the calls.
    fn counting_wrap(calls: &Cell<usize>) -> impl Fn(&String, usize) -> Vec<String> + '_ {
        move |line: &String, width: usize| {
            calls.set(calls.get() + 1);
            wrap_rows(line, width)
                .into_iter()
                .map(|range| line[range].to_owned())
                .collect()
        }
    }

    #[test]
    fn the_limit_evicts_the_oldest_lines() {
        let calls = Cell::new(0);
        let mut scrollback = Scrollback::with_limit(3, counting_wrap(&calls));
        for n in 0..5 {
            scrollback.push(format!("line {n}"));
        }
        assert_eq!(scrollback.len(), 3);
        assert_eq!(scrollback.evicted(), 2);
        let lines: Vec<&String> = scrollback.lines().collect();
        assert_eq!(lines, ["line 2", "line 3", "line 4"]);
    }

    #[test]
    fn a_frame_wraps_only_the_lines_it_has_not_seen() {
        let calls = Cell::new(0);
        let mut scrollback = Scrollback::new(counting_wrap(&calls));
        for n in 0..100 {
            scrollback.push(format!("line {n}"));
        }
        assert_eq!(scrollback.rows(80, |_| true).len(), 100);
        assert_eq!(calls.get(), 100);
        assert_eq!(scrollback.rows(80, |_| true).len(), 100);
        assert_eq!(
            calls.get(),
            100,
            "a second frame at the same width wraps nothing"
        );
        scrollback.push("one more".to_owned());
        assert_eq!(scrollback.rows(80, |_| true).len(), 101);
        assert_eq!(calls.get(), 101);
    }

    #[test]
    fn a_resize_rewraps_every_line() {
        let calls = Cell::new(0);
        let mut scrollback = Scrollback::new(counting_wrap(&calls));
        scrollback.push("the quick brown fox jumps over the lazy dog".to_owned());
        scrollback.push("short".to_owned());
        assert_eq!(scrollback.rows(80, |_| true).len(), 2);
        assert_eq!(calls.get(), 2);
        let narrow: Vec<String> = scrollback.rows(10, |_| true).into_iter().cloned().collect();
        assert_eq!(calls.get(), 4);
        assert_eq!(
            narrow,
            [
                "the quick",
                "brown fox",
                "jumps",
                "over the",
                "lazy dog",
                "short"
            ]
        );
        assert_eq!(scrollback.rows(80, |_| true).len(), 2);
        assert_eq!(calls.get(), 6);
    }

    #[test]
    fn a_filter_hides_lines_without_wrapping_them() {
        let calls = Cell::new(0);
        let mut scrollback = Scrollback::new(counting_wrap(&calls));
        scrollback.push("shown".to_owned());
        scrollback.push("detail".to_owned());
        let rows: Vec<String> = scrollback
            .rows(80, |line| line != "detail")
            .into_iter()
            .cloned()
            .collect();
        assert_eq!(rows, ["shown"]);
        assert_eq!(calls.get(), 1);
        assert_eq!(scrollback.rows(80, |_| true).len(), 2);
        assert_eq!(calls.get(), 2, "revealing the detail wraps it once");
    }
}
