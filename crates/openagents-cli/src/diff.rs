//! The diff inspector: what changed, side by side or unified.
//!
//! Three parts, and the split matters because two of them are pure.
//!
//! - [`compare`] computes a diff between two texts. It is Myers' algorithm
//!   over lines, with a bounded edit distance and a stated fallback, so a
//!   pathological pair cannot hang the session that called it.
//! - [`parse_unified`] reads a diff somebody else produced — `git diff`
//!   output — into the same shape. A diff the tool already knows about is
//!   better than one recomputed from files that have since moved on.
//! - [`render`] turns a [`FileDiff`] into rows. Unified and side-by-side are
//!   two functions over one model rather than two models.
//!
//! Nothing here touches the terminal or the filesystem, so every rendering
//! below is asserted in this file against the rows a reader would see.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr as _;

use crate::markdown::truncate_spans;

/// What happened to one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Equal,
    Delete,
    Insert,
}

/// One line of a hunk, with the line numbers it holds on each side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub tag: Tag,
    /// Its number in the old file, if it is in the old file.
    pub old: Option<usize>,
    /// Its number in the new file, if it is in the new file.
    pub new: Option<usize>,
    pub text: String,
}

/// A run of changed lines and the context around it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    /// The `@@ -a,b +c,d @@` header this hunk would be written with.
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_count, self.new_start, self.new_count
        )
    }
}

/// Every hunk for one path.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FileDiff {
    /// The path as it is now. A deleted file keeps the path it had.
    pub path: String,
    /// The path it had before, when a rename moved it.
    pub renamed_from: Option<String>,
    pub hunks: Vec<Hunk>,
    /// A file whose difference this tool will not show: a binary one, or one
    /// git reported without a body. Carries the reason, which is printed
    /// instead of an empty pane.
    pub note: Option<String>,
}

impl FileDiff {
    /// Lines added and lines removed.
    pub fn stats(&self) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line.tag {
                    Tag::Insert => added += 1,
                    Tag::Delete => removed += 1,
                    Tag::Equal => {}
                }
            }
        }
        (added, removed)
    }

    pub fn is_empty(&self) -> bool {
        self.hunks.iter().all(|hunk| hunk.lines.is_empty())
    }
}

/// How the inspector lays a diff out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMode {
    Unified,
    SideBySide,
}

impl DiffMode {
    pub fn toggled(self) -> Self {
        match self {
            DiffMode::Unified => DiffMode::SideBySide,
            DiffMode::SideBySide => DiffMode::Unified,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DiffMode::Unified => "unified",
            DiffMode::SideBySide => "side by side",
        }
    }
}

// ------------------------------------------------------------- computing one

/// Rows of context kept either side of a change.
pub const CONTEXT: usize = 3;

/// The largest edit distance the line diff will search before it gives up.
///
/// Myers' algorithm costs O(ND) in the edit distance, and the trace this one
/// keeps in order to backtrack costs O(D²) in memory. Two unrelated files are
/// the expensive case, and they are also the case where a line-by-line diff
/// tells the reader nothing: past this bound the answer is reported as a
/// wholesale replacement, which is true, cheap, and no less legible.
///
/// The bound is on the differing middle. Matching head and tail are trimmed
/// before the search, so a one-line edit in a fifty-thousand-line file is
/// nowhere near it.
const MAX_EDIT_DISTANCE: usize = 1_500;

/// Diff two texts by line.
pub fn compare(old: &str, new: &str, context: usize) -> Vec<Hunk> {
    let old_lines: Vec<&str> = split(old);
    let new_lines: Vec<&str> = split(new);
    let script = myers(&old_lines, &new_lines);
    hunks(script, context)
}

fn split(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    // A trailing newline ends the last line; it does not begin an empty one.
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// Myers' greedy algorithm, producing one [`DiffLine`] per line of both files.
fn myers(old: &[&str], new: &[&str]) -> Vec<DiffLine> {
    let n = old.len();
    let m = new.len();

    // Common head and tail are not worth searching through, and trimming them
    // is what keeps the search small for the ordinary case of a small edit in
    // a large file.
    let head = old
        .iter()
        .zip(new.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let mut script = Vec::with_capacity(n.max(m));
    for (index, line) in old[..head].iter().enumerate() {
        script.push(DiffLine {
            tag: Tag::Equal,
            old: Some(index + 1),
            new: Some(index + 1),
            text: (*line).to_string(),
        });
    }

    let middle_old = &old[head..n - tail];
    let middle_new = &new[head..m - tail];
    let middle = match trace(middle_old, middle_new) {
        Some(path) => walk(path, middle_old, middle_new, head),
        // Past the bound, or two texts with nothing in common: everything
        // that was there went, and everything that is there arrived.
        None => wholesale(middle_old, middle_new, head),
    };
    script.extend(middle);

    for offset in 0..tail {
        script.push(DiffLine {
            tag: Tag::Equal,
            old: Some(n - tail + offset + 1),
            new: Some(m - tail + offset + 1),
            text: old[n - tail + offset].to_string(),
        });
    }
    script
}

fn wholesale(old: &[&str], new: &[&str], head: usize) -> Vec<DiffLine> {
    let mut out = Vec::with_capacity(old.len() + new.len());
    for (index, line) in old.iter().enumerate() {
        out.push(DiffLine {
            tag: Tag::Delete,
            old: Some(head + index + 1),
            new: None,
            text: (*line).to_string(),
        });
    }
    for (index, line) in new.iter().enumerate() {
        out.push(DiffLine {
            tag: Tag::Insert,
            old: None,
            new: Some(head + index + 1),
            text: (*line).to_string(),
        });
    }
    out
}

/// The furthest-reaching endpoint on each diagonal, for each edit distance.
///
/// A *diagonal* `k` is the set of points where `x - y == k`. `trace[d]` is the
/// frontier as it stood *before* the `d`-th step: for each diagonal, how far
/// along the old file the best path had reached. [`walk`] reads it backwards
/// to recover the edits.
///
/// Each row holds only the diagonals that step can touch, `-(d+1)..=(d+1)`,
/// indexed by [`slot`]. That keeps the trace quadratic in the edit distance
/// rather than proportional to the size of the files, which for a small edit
/// in a large file is the whole difference.
///
/// `None` means the bound was reached, which the caller answers by reporting
/// a wholesale replacement rather than by searching on.
fn trace(old: &[&str], new: &[&str]) -> Option<Vec<Vec<usize>>> {
    let n = old.len();
    let m = new.len();
    let max = n + m;
    if max == 0 {
        return Some(Vec::new());
    }

    // One slot per diagonal from `-(max+1)` to `max+1`, so the neighbours of
    // the outermost diagonal are addressable without a bounds check.
    let mut frontier = vec![0usize; 2 * max + 3];
    let width = |d: isize| (max as isize + 1 - (d + 1)) as usize..(max + 1 + (d as usize + 1)) + 1;
    let mut trace: Vec<Vec<usize>> = Vec::new();

    for d in 0..=max.min(MAX_EDIT_DISTANCE) as isize {
        trace.push(frontier[width(d)].to_vec());
        let mut k = -d;
        while k <= d {
            // Take whichever neighbour reaches further: down from `k + 1` is
            // an insertion, right from `k - 1` is a deletion. At the edges of
            // the frontier only one of the two exists.
            let down =
                k == -d || (k != d && frontier[offset(k - 1, max)] < frontier[offset(k + 1, max)]);
            let mut x = if down {
                frontier[offset(k + 1, max)]
            } else {
                frontier[offset(k - 1, max)] + 1
            };
            let mut y = (x as isize - k) as usize;
            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }
            frontier[offset(k, max)] = x;
            if x >= n && y >= m {
                return Some(trace);
            }
            k += 2;
        }
    }
    None
}

/// Where diagonal `k` lives in the whole frontier.
fn offset(k: isize, max: usize) -> usize {
    (k + max as isize + 1) as usize
}

/// Where diagonal `k` lives in `trace[d]`, which holds only `-(d+1)..=(d+1)`.
fn slot(k: isize, d: isize) -> usize {
    (k + d + 1) as usize
}

/// Backtrack the trace into one entry per line of both files.
fn walk(trace: Vec<Vec<usize>>, old: &[&str], new: &[&str], head: usize) -> Vec<DiffLine> {
    let n = old.len();
    let m = new.len();
    let mut out: Vec<DiffLine> = Vec::new();
    if n + m == 0 {
        return out;
    }

    let mut x = n;
    let mut y = m;
    for d in (0..trace.len()).rev() {
        let v = &trace[d];
        let d = d as isize;
        let k = x as isize - y as isize;
        let down = k == -d || (k != d && v[slot(k - 1, d)] < v[slot(k + 1, d)]);
        let previous_k = if down { k + 1 } else { k - 1 };
        let previous_x = v[slot(previous_k, d)];
        let previous_y = (previous_x as isize - previous_k) as usize;

        // The run of matching lines this step ended on.
        while x > previous_x && y > previous_y {
            x -= 1;
            y -= 1;
            out.push(DiffLine {
                tag: Tag::Equal,
                old: Some(head + x + 1),
                new: Some(head + y + 1),
                text: old[x].to_string(),
            });
        }
        if d == 0 {
            break;
        }
        // Then the single edit that got there.
        if x > previous_x {
            x -= 1;
            out.push(DiffLine {
                tag: Tag::Delete,
                old: Some(head + x + 1),
                new: None,
                text: old[x].to_string(),
            });
        } else if y > previous_y {
            y -= 1;
            out.push(DiffLine {
                tag: Tag::Insert,
                old: None,
                new: Some(head + y + 1),
                text: new[y].to_string(),
            });
        }
        x = previous_x;
        y = previous_y;
    }
    out.reverse();
    out
}

/// Group a full edit script into hunks with `context` lines either side.
fn hunks(script: Vec<DiffLine>, context: usize) -> Vec<Hunk> {
    let changed: Vec<usize> = script
        .iter()
        .enumerate()
        .filter(|(_, line)| line.tag != Tag::Equal)
        .map(|(index, _)| index)
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }

    // Walk the changed indices, starting a new hunk whenever the gap between
    // two changes is wider than twice the context — any narrower and the two
    // runs of context would touch, and one hunk reads better than two.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut start = changed[0];
    let mut end = changed[0];
    for index in changed.into_iter().skip(1) {
        if index - end > context * 2 + 1 {
            spans.push((start, end));
            start = index;
        }
        end = index;
    }
    spans.push((start, end));

    spans
        .into_iter()
        .map(|(start, end)| {
            let from = start.saturating_sub(context);
            let to = (end + context + 1).min(script.len());
            let lines: Vec<DiffLine> = script[from..to].to_vec();
            let old_count = lines.iter().filter(|l| l.tag != Tag::Insert).count();
            let new_count = lines.iter().filter(|l| l.tag != Tag::Delete).count();
            Hunk {
                old_start: lines.iter().find_map(|l| l.old).unwrap_or(0),
                old_count,
                new_start: lines.iter().find_map(|l| l.new).unwrap_or(0),
                new_count,
                lines,
            }
        })
        .collect()
}

// ------------------------------------------------------------------ parsing

/// Read `git diff` output into file diffs.
///
/// Every line number in the result comes from the `@@` headers, so a hunk that
/// git elided context from still reports the file's own numbering.
pub fn parse_unified(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for raw in text.split('\n') {
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            files.push(FileDiff {
                path: git_paths(rest).map_or_else(|| rest.to_string(), |(_, b)| b),
                ..FileDiff::default()
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if let Some(rest) = raw.strip_prefix("rename from ") {
            file.renamed_from = Some(rest.to_string());
            continue;
        }
        if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
            file.note = Some("Binary file. Nothing to show line by line.".to_string());
            continue;
        }
        if let Some(rest) = raw.strip_prefix("+++ b/") {
            file.path = rest.to_string();
            continue;
        }
        if raw.starts_with("@@") {
            if let Some((os, oc, ns, nc)) = hunk_header(raw) {
                old_line = os;
                new_line = ns;
                file.hunks.push(Hunk {
                    old_start: os,
                    old_count: oc,
                    new_start: ns,
                    new_count: nc,
                    lines: Vec::new(),
                });
            }
            continue;
        }
        let Some(hunk) = file.hunks.last_mut() else {
            continue;
        };
        // `\ No newline at end of file` annotates the line before it; it is
        // not a line of either side.
        if raw.starts_with('\\') {
            continue;
        }
        match raw.chars().next() {
            Some('+') => {
                hunk.lines.push(DiffLine {
                    tag: Tag::Insert,
                    old: None,
                    new: Some(new_line),
                    text: raw[1..].to_string(),
                });
                new_line += 1;
            }
            Some('-') => {
                hunk.lines.push(DiffLine {
                    tag: Tag::Delete,
                    old: Some(old_line),
                    new: None,
                    text: raw[1..].to_string(),
                });
                old_line += 1;
            }
            Some(' ') => {
                hunk.lines.push(DiffLine {
                    tag: Tag::Equal,
                    old: Some(old_line),
                    new: Some(new_line),
                    text: raw[1..].to_string(),
                });
                old_line += 1;
                new_line += 1;
            }
            // An empty line inside a hunk is an unchanged empty line whose
            // leading space some tools trim.
            None => hunk.lines.push(DiffLine {
                tag: Tag::Equal,
                old: Some(old_line),
                new: Some(new_line),
                text: String::new(),
            }),
            _ => {}
        }
        if raw.is_empty() {
            old_line += 1;
            new_line += 1;
        }
    }
    files
}

/// `a/path b/path` from a `diff --git` line, as (old, new).
fn git_paths(rest: &str) -> Option<(String, String)> {
    let (a, b) = rest.split_once(" b/")?;
    let a = a.strip_prefix("a/")?;
    Some((a.to_string(), b.to_string()))
}

/// `@@ -12,7 +12,9 @@` as its four numbers. A count is 1 when it is omitted.
fn hunk_header(raw: &str) -> Option<(usize, usize, usize, usize)> {
    let body = raw.strip_prefix("@@ ")?;
    let body = body.split(" @@").next()?;
    let (old, new) = body.split_once(' ')?;
    let old = pair(old.strip_prefix('-')?)?;
    let new = pair(new.strip_prefix('+')?)?;
    Some((old.0, old.1, new.0, new.1))
}

fn pair(text: &str) -> Option<(usize, usize)> {
    match text.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

// ---------------------------------------------------------------- rendering

fn added_style() -> Style {
    Style::default().fg(Color::Green)
}
fn removed_style() -> Style {
    Style::default().fg(Color::Red)
}
fn number_style() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn rule_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// The narrowest body either column of the side-by-side view is worth having.
///
/// Below this the two columns show an ellipsis each, which says less than one
/// column of real text, so the view falls back to unified and says so.
const MIN_SIDE_BY_SIDE_BODY: usize = 8;

/// Every row of a file's diff, in the mode asked for.
pub fn render(file: &FileDiff, mode: DiffMode, width: usize) -> Vec<Line<'static>> {
    let width = width.max(8);
    let mut rows = vec![header_row(file)];

    if let Some(note) = &file.note {
        rows.push(Line::from(Span::styled(
            note.clone(),
            Style::default().fg(Color::Yellow),
        )));
        return rows;
    }
    if file.is_empty() {
        rows.push(Line::from(Span::styled(
            "No line changes.".to_string(),
            number_style(),
        )));
        return rows;
    }

    // One number width for the whole file, so the columns line up across
    // hunks rather than shifting at every `@@`.
    let numbers = file.hunks.iter().map(number_width).max().unwrap_or(2);
    let mode = match mode {
        DiffMode::SideBySide if side_by_side_body(width, numbers) < MIN_SIDE_BY_SIDE_BODY => {
            rows.push(Line::from(Span::styled(
                "Too narrow for two columns. Showing the unified view.".to_string(),
                Style::default().fg(Color::Yellow),
            )));
            DiffMode::Unified
        }
        mode => mode,
    };

    for hunk in &file.hunks {
        rows.push(Line::from(Span::styled(hunk.header(), rule_style())));
        match mode {
            DiffMode::Unified => rows.extend(unified_rows(hunk, numbers, width)),
            DiffMode::SideBySide => rows.extend(side_by_side_rows(hunk, numbers, width)),
        }
    }

    // The last word on the width. The body rows are already cut to fit, but a
    // long path in the header or a note wider than the pane would otherwise be
    // truncated by the renderer without an ellipsis to say so.
    rows.into_iter()
        .map(|row| Line::from(truncate_spans(row.spans, width)))
        .collect()
}

fn header_row(file: &FileDiff) -> Line<'static> {
    let (added, removed) = file.stats();
    let mut spans = vec![Span::styled(
        file.path.clone(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(from) = &file.renamed_from {
        spans.push(Span::styled(format!("  ← {from}"), number_style()));
    }
    spans.push(Span::styled("  +".to_string(), added_style()));
    spans.push(Span::styled(added.to_string(), added_style()));
    spans.push(Span::styled(" −".to_string(), removed_style()));
    spans.push(Span::styled(removed.to_string(), removed_style()));
    Line::from(spans)
}

/// How many columns a line number takes, given the largest one in the hunk.
fn number_width(hunk: &Hunk) -> usize {
    let largest = hunk
        .lines
        .iter()
        .filter_map(|line| line.old.max(line.new))
        .max()
        .unwrap_or(0);
    largest.to_string().len().max(2)
}

fn number(value: Option<usize>, width: usize) -> String {
    match value {
        Some(n) => format!("{n:>width$}"),
        None => " ".repeat(width),
    }
}

fn sign(tag: Tag) -> (&'static str, Style) {
    match tag {
        Tag::Insert => ("+", added_style()),
        Tag::Delete => ("−", removed_style()),
        Tag::Equal => (" ", Style::default()),
    }
}

/// `12  13 │+ the line`
fn unified_rows(hunk: &Hunk, numbers: usize, width: usize) -> Vec<Line<'static>> {
    // Two numbers, a space between them, the rule, the sign, and a space.
    let gutter = numbers * 2 + 1 + 2 + 2;
    let body = width.saturating_sub(gutter).max(4);

    hunk.lines
        .iter()
        .map(|line| {
            let (mark, style) = sign(line.tag);
            let mut spans = vec![
                Span::styled(
                    format!(
                        "{} {}",
                        number(line.old, numbers),
                        number(line.new, numbers)
                    ),
                    number_style(),
                ),
                Span::styled(" │".to_string(), rule_style()),
                Span::styled(format!("{mark} "), style),
            ];
            spans.extend(truncate_spans(
                vec![Span::styled(expand_tabs(&line.text), style)],
                body,
            ));
            Line::from(spans)
        })
        .collect()
}

/// One row of the side-by-side layout: the old line and the new one.
///
/// A run of removals and the run of additions that replaced it are zipped, so
/// a changed line sits opposite the line it changed from. A run with no
/// opposite number leaves that side blank.
fn pair_rows(hunk: &Hunk) -> Vec<(Option<&DiffLine>, Option<&DiffLine>)> {
    let mut rows = Vec::new();
    let mut removed: Vec<&DiffLine> = Vec::new();
    let mut added: Vec<&DiffLine> = Vec::new();

    fn flush<'a>(
        removed: &mut Vec<&'a DiffLine>,
        added: &mut Vec<&'a DiffLine>,
        rows: &mut Vec<(Option<&'a DiffLine>, Option<&'a DiffLine>)>,
    ) {
        for index in 0..removed.len().max(added.len()) {
            rows.push((removed.get(index).copied(), added.get(index).copied()));
        }
        removed.clear();
        added.clear();
    }

    for line in &hunk.lines {
        match line.tag {
            Tag::Delete => removed.push(line),
            Tag::Insert => added.push(line),
            Tag::Equal => {
                flush(&mut removed, &mut added, &mut rows);
                rows.push((Some(line), Some(line)));
            }
        }
    }
    flush(&mut removed, &mut added, &mut rows);
    rows
}

/// How many columns of text each side of the split view gets.
///
/// Each side spends `numbers` on its line number, two on the rule, and two on
/// the sign; the two sides are separated by a rule of their own.
fn side_by_side_body(width: usize, numbers: usize) -> usize {
    (width.saturating_sub(3) / 2).saturating_sub(numbers + 4)
}

fn side_by_side_rows(hunk: &Hunk, numbers: usize, width: usize) -> Vec<Line<'static>> {
    let body = side_by_side_body(width, numbers);

    pair_rows(hunk)
        .into_iter()
        .map(|(old, new)| {
            let mut spans = column(old, |line| line.old, numbers, body);
            spans.push(Span::styled(" │ ".to_string(), rule_style()));
            spans.extend(column(new, |line| line.new, numbers, body));
            Line::from(spans)
        })
        .collect()
}

/// One side of a side-by-side row, padded to a fixed width so the divider
/// between the two columns stays in one place down the whole pane.
fn column(
    line: Option<&DiffLine>,
    pick: fn(&DiffLine) -> Option<usize>,
    numbers: usize,
    body: usize,
) -> Vec<Span<'static>> {
    let Some(line) = line else {
        return vec![Span::raw(" ".repeat(numbers + 2 + 2 + body))];
    };
    let (mark, style) = sign(line.tag);
    let text = expand_tabs(&line.text);
    let drawn = text.width().min(body);
    let mut spans = vec![
        Span::styled(number(pick(line), numbers), number_style()),
        Span::styled(" │".to_string(), rule_style()),
        Span::styled(format!("{mark} "), style),
    ];
    spans.extend(truncate_spans(vec![Span::styled(text, style)], body));
    if drawn < body {
        spans.push(Span::raw(" ".repeat(body - drawn)));
    }
    spans
}

/// Tabs are not a width the renderer can reason about, so they become spaces.
fn expand_tabs(text: &str) -> String {
    text.replace('\t', "    ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(rows: &[Line<'_>]) -> Vec<String> {
        rows.iter().map(text).collect()
    }

    fn tags(hunks: &[Hunk]) -> Vec<(Tag, String)> {
        hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .map(|l| (l.tag, l.text.clone()))
            .collect()
    }

    // ------------------------------------------------------------ computing

    #[test]
    fn identical_texts_have_no_hunks() {
        assert!(compare("a\nb\nc\n", "a\nb\nc\n", CONTEXT).is_empty());
    }

    #[test]
    fn one_changed_line_is_one_delete_and_one_insert() {
        let hunks = compare("a\nb\nc\n", "a\nB\nc\n", CONTEXT);
        assert_eq!(
            tags(&hunks),
            vec![
                (Tag::Equal, "a".to_string()),
                (Tag::Delete, "b".to_string()),
                (Tag::Insert, "B".to_string()),
                (Tag::Equal, "c".to_string()),
            ]
        );
    }

    #[test]
    fn line_numbers_are_each_sides_own() {
        let hunks = compare("a\nb\n", "a\nx\ny\nb\n", CONTEXT);
        let lines = &hunks[0].lines;
        let inserted: Vec<_> = lines
            .iter()
            .filter(|l| l.tag == Tag::Insert)
            .map(|l| (l.new, l.old))
            .collect();
        assert_eq!(inserted, vec![(Some(2), None), (Some(3), None)]);
        let last = lines.last().expect("a trailing context line");
        assert_eq!((last.old, last.new), (Some(2), Some(4)));
    }

    #[test]
    fn an_insertion_at_the_top_keeps_the_lines_below_it() {
        let hunks = compare("b\nc\n", "a\nb\nc\n", CONTEXT);
        assert_eq!(
            tags(&hunks),
            vec![
                (Tag::Insert, "a".to_string()),
                (Tag::Equal, "b".to_string()),
                (Tag::Equal, "c".to_string()),
            ]
        );
    }

    #[test]
    fn a_deletion_at_the_end_is_a_deletion_not_a_rewrite() {
        let hunks = compare("a\nb\nc\n", "a\nb\n", CONTEXT);
        assert_eq!(
            tags(&hunks),
            vec![
                (Tag::Equal, "a".to_string()),
                (Tag::Equal, "b".to_string()),
                (Tag::Delete, "c".to_string()),
            ]
        );
    }

    /// The property that matters more than any single shape: applying the
    /// script to the old text has to produce the new text exactly.
    #[test]
    fn the_script_turns_the_old_text_into_the_new_one() {
        let twenty = "x\n".repeat(20);
        let nineteen = "x\n".repeat(19);
        let cases = [
            ("", "a\nb\n"),
            ("a\nb\n", ""),
            ("a\nb\nc\nd\ne\n", "a\nc\nb\nd\nx\n"),
            ("one\ntwo\nthree\n", "one\nTWO\nthree\nfour\n"),
            (twenty.as_str(), nineteen.as_str()),
            (
                "alpha\nbeta\ngamma\ndelta\n",
                "gamma\ndelta\nalpha\nbeta\nepsilon\n",
            ),
        ];
        for (old, new) in cases {
            // Rebuild the new file from the whole script, not from the hunks:
            // hunks drop unchanged runs, which is the point of them.
            let script = myers(&split(old), &split(new));
            let rebuilt: Vec<&str> = script
                .iter()
                .filter(|line| line.tag != Tag::Delete)
                .map(|line| line.text.as_str())
                .collect();
            assert_eq!(rebuilt, split(new), "old={old:?} new={new:?}");

            let kept: Vec<&str> = script
                .iter()
                .filter(|line| line.tag != Tag::Insert)
                .map(|line| line.text.as_str())
                .collect();
            assert_eq!(kept, split(old), "old={old:?} new={new:?}");
        }
    }

    #[test]
    fn far_apart_changes_get_a_hunk_each_and_near_ones_share() {
        let old: String = (1..=40).map(|n| format!("line {n}\n")).collect();
        let mut changed: Vec<String> = (1..=40).map(|n| format!("line {n}")).collect();
        changed[2] = "changed near the top".to_string();
        changed[35] = "changed near the bottom".to_string();
        let new = format!("{}\n", changed.join("\n"));
        assert_eq!(compare(&old, &new, CONTEXT).len(), 2);

        let mut close: Vec<String> = (1..=40).map(|n| format!("line {n}")).collect();
        close[2] = "one".to_string();
        close[5] = "two".to_string();
        let near = format!("{}\n", close.join("\n"));
        assert_eq!(compare(&old, &near, CONTEXT).len(), 1);
    }

    #[test]
    fn context_is_kept_either_side_of_a_change() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old.replace("line 10\n", "LINE TEN\n");
        let hunks = compare(&old, &new, CONTEXT);
        assert_eq!(hunks.len(), 1);
        let hunk = &hunks[0];
        assert_eq!(hunk.old_start, 7, "{:?}", hunk.lines);
        assert_eq!(hunk.lines.first().map(|l| l.text.as_str()), Some("line 7"));
        assert_eq!(hunk.lines.last().map(|l| l.text.as_str()), Some("line 13"));
    }

    // -------------------------------------------------------------- parsing

    const GIT_OUTPUT: &str = "\
diff --git a/lib/thing.ex b/lib/thing.ex
index 1111111..2222222 100644
--- a/lib/thing.ex
+++ b/lib/thing.ex
@@ -12,6 +12,7 @@ defmodule Thing do
   def run do
     :ok
   end
+  def extra, do: :new

   def other do
-    :old
+    :changed
   end
";

    #[test]
    fn git_output_parses_into_one_file_with_its_own_line_numbers() {
        let files = parse_unified(GIT_OUTPUT);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "lib/thing.ex");
        assert_eq!(files[0].stats(), (2, 1));

        let inserted: Vec<_> = files[0].hunks[0]
            .lines
            .iter()
            .filter(|l| l.tag == Tag::Insert)
            .map(|l| (l.new, l.text.clone()))
            .collect();
        assert_eq!(
            inserted,
            vec![
                (Some(15), "  def extra, do: :new".to_string()),
                (Some(18), "    :changed".to_string()),
            ]
        );
    }

    #[test]
    fn two_files_in_one_diff_stay_two_files() {
        let both = format!(
            "{GIT_OUTPUT}diff --git a/README.md b/README.md\n\
             --- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n"
        );
        let files = parse_unified(&both);
        assert_eq!(
            files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            vec!["lib/thing.ex", "README.md"]
        );
        assert_eq!(files[1].stats(), (1, 1));
    }

    #[test]
    fn a_binary_file_says_so_rather_than_showing_an_empty_pane() {
        let files = parse_unified(
            "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n",
        );
        let rows = render(&files[0], DiffMode::Unified, 60);
        assert!(
            texts(&rows).iter().any(|row| row.contains("Binary file")),
            "{:?}",
            texts(&rows)
        );
    }

    #[test]
    fn a_rename_reports_the_path_it_came_from() {
        let files = parse_unified(
            "diff --git a/old/name.rs b/new/name.rs\nsimilarity index 98%\n\
             rename from old/name.rs\nrename to new/name.rs\n",
        );
        assert_eq!(files[0].path, "new/name.rs");
        assert_eq!(files[0].renamed_from.as_deref(), Some("old/name.rs"));
        assert!(text(&render(&files[0], DiffMode::Unified, 60)[0]).contains("old/name.rs"));
    }

    // ------------------------------------------------------------ rendering

    fn one_change() -> FileDiff {
        FileDiff {
            path: "src/lib.rs".to_string(),
            hunks: compare("keep\nold line\ntail\n", "keep\nnew line\ntail\n", CONTEXT),
            ..FileDiff::default()
        }
    }

    #[test]
    fn the_unified_view_shows_both_numbers_a_sign_and_the_text() {
        let rows = render(&one_change(), DiffMode::Unified, 46);
        let drawn = texts(&rows);
        assert_eq!(drawn[0], "src/lib.rs  +1 −1");
        assert_eq!(drawn[1], "@@ -1,3 +1,3 @@");
        assert_eq!(drawn[2], " 1  1 │  keep");
        assert_eq!(drawn[3], " 2    │− old line");
        assert_eq!(drawn[4], "    2 │+ new line");
        assert_eq!(drawn[5], " 3  3 │  tail");
    }

    #[test]
    fn the_side_by_side_view_puts_the_change_opposite_what_it_replaced() {
        let rows = render(&one_change(), DiffMode::SideBySide, 46);
        let drawn = texts(&rows);
        assert_eq!(drawn[2], " 1 │  keep            │  1 │  keep           ");
        assert_eq!(drawn[3], " 2 │− old line        │  2 │+ new line       ");
        assert_eq!(drawn[4], " 3 │  tail            │  3 │  tail           ");
    }

    /// A run of three removals replaced by one addition leaves two rows with
    /// nothing on the right, rather than sliding the rest of the file up.
    #[test]
    fn an_unequal_run_leaves_the_short_side_blank() {
        let file = FileDiff {
            path: "f".to_string(),
            hunks: compare("a\nb\nc\nd\n", "a\nZ\nd\n", CONTEXT),
            ..FileDiff::default()
        };
        let rows = texts(&render(&file, DiffMode::SideBySide, 40));
        // Row 0 is the header, row 1 the hunk header, row 2 the `a` context.
        assert!(rows[3].contains('b') && rows[3].contains('Z'), "{rows:?}");
        assert!(rows[4].contains('c'), "{rows:?}");
        assert!(!rows[4].contains('Z'), "{rows:?}");
        assert!(
            rows[4].ends_with("   "),
            "the right column was not left blank: {:?}",
            rows[4]
        );
    }

    #[test]
    fn no_rendered_row_is_wider_than_the_pane() {
        let long = format!("{}\n", "x".repeat(400));
        let file = FileDiff {
            path: "wide".to_string(),
            hunks: compare(
                &format!("a\n{long}b\n"),
                &format!("a\n{}\nb\n", "y".repeat(400)),
                CONTEXT,
            ),
            ..FileDiff::default()
        };
        for mode in [DiffMode::Unified, DiffMode::SideBySide] {
            for width in [20usize, 33, 60, 120] {
                for row in render(&file, mode, width) {
                    let drawn: usize = row.spans.iter().map(|s| s.content.width()).sum();
                    assert!(
                        drawn <= width,
                        "{mode:?} drew {drawn} columns into {width}: {:?}",
                        text(&row)
                    );
                }
            }
        }
    }

    #[test]
    fn additions_are_green_and_removals_are_red() {
        let rows = render(&one_change(), DiffMode::Unified, 46);
        let removed = rows[3]
            .spans
            .iter()
            .find(|s| s.content.contains("old line"))
            .and_then(|s| s.style.fg);
        let added = rows[4]
            .spans
            .iter()
            .find(|s| s.content.contains("new line"))
            .and_then(|s| s.style.fg);
        assert_eq!(removed, Some(Color::Red));
        assert_eq!(added, Some(Color::Green));
    }

    #[test]
    fn the_mode_toggles_both_ways_and_names_itself() {
        assert_eq!(DiffMode::Unified.toggled(), DiffMode::SideBySide);
        assert_eq!(DiffMode::SideBySide.toggled(), DiffMode::Unified);
        assert_eq!(DiffMode::Unified.label(), "unified");
        assert_eq!(DiffMode::SideBySide.label(), "side by side");
    }
}
