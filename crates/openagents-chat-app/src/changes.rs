//! Read-only "What changed" card and unified diff.
//!
//! Reimplemented from Zeron's diff pane (public MIT zeronsh/zeron): one
//! unified view, virtualized by line, with paint-only syntax spans. Opening
//! the card shows the change. Nothing here can edit it.

use rust_native::syntax::{Highlighter, Span};

/// The most lines one change document keeps.
pub const MAX_LINES: usize = 20_000;
/// The most bytes of diff text one document reads.
pub const MAX_BYTES: usize = 1024 * 1024;
/// The most bytes of one displayed line.
const MAX_LINE: usize = 2_000;
/// Visible lines drawn for one viewport, including one line of overscan.
const WINDOW_CAP: usize = 80;

/// What a displayed line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A `diff --git` file header.
    File,
    /// A hunk header.
    Hunk,
    /// An added line.
    Add,
    /// A removed line.
    Remove,
    /// An unchanged line.
    Context,
    /// A Git header or note that is not a changed line.
    Meta,
}

/// One displayed line. Spans are paint-only: filling them does not change
/// the text or the line count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    /// File extension the highlighter understands, such as `rs`.
    pub language: String,
    /// Foreground spans into `text`. `None` means not highlighted yet.
    pub spans: Option<Vec<Span>>,
}

/// A parsed unified diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    lines: Vec<Line>,
    files: usize,
    added: u32,
    removed: u32,
}

impl Document {
    /// How many lines the pane can scroll through.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the document has nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The lines, in file order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The card's second line: file count and added and removed counts.
    #[must_use]
    pub fn summary(&self) -> String {
        let files = match self.files {
            1 => "1 file".to_owned(),
            count => format!("{count} files"),
        };
        format!("{files}, +{}, −{}", self.added, self.removed)
    }

    /// The first visible line and how many lines fit in `viewport` points.
    ///
    /// `line` is the uniform line height. The result never walks the lines
    /// and never exceeds [`WINDOW_CAP`].
    #[must_use]
    pub fn window(&self, scroll: f32, viewport: f32, line: f32) -> (usize, usize) {
        let total = self.lines.len();
        if total == 0
            || !viewport.is_finite()
            || viewport <= 0.0
            || !line.is_finite()
            || line <= 0.0
        {
            return (0, 0);
        }
        let scroll = scroll.clamp(0.0, self.scroll_limit(viewport, line));
        let first = (scroll / line).floor() as usize;
        let count = ((viewport / line).ceil() as usize)
            .saturating_add(1)
            .clamp(1, WINDOW_CAP);
        let first = first.min(total - 1);
        (first, count.min(total - first))
    }

    /// How far the pane can scroll, in the same points as `viewport`.
    #[must_use]
    pub fn scroll_limit(&self, viewport: f32, line: f32) -> f32 {
        if !line.is_finite() || line <= 0.0 {
            return 0.0;
        }
        (self.lines.len() as f32 * line - viewport.max(0.0)).max(0.0)
    }

    /// Fill syntax spans for the visible lines. Headers stay plain.
    pub fn ensure_spans(&mut self, first: usize, count: usize, highlighter: &Highlighter) {
        let end = first.saturating_add(count).min(self.lines.len());
        let start = first.min(self.lines.len());
        for line in &mut self.lines[start..end] {
            if line.spans.is_some() {
                continue;
            }
            line.spans = Some(highlight(line, highlighter));
        }
    }
}

/// Parse a unified diff. Text past the line or byte bound is omitted, and
/// the document says so on its last line.
#[must_use]
pub fn parse(diff: &str) -> Document {
    let mut doc = Document {
        lines: Vec::new(),
        files: 0,
        added: 0,
        removed: 0,
    };
    let mut language = String::new();
    let mut bytes = 0usize;
    let mut cut = false;
    for line in diff.split_inclusive('\n') {
        bytes = bytes.saturating_add(line.len());
        if doc.lines.len() >= MAX_LINES || bytes > MAX_BYTES {
            cut = true;
            break;
        }
        let raw = line.trim_end_matches(['\n', '\r']);
        if raw.is_empty() && doc.lines.is_empty() {
            continue;
        }
        let kind = classify(raw);
        if kind == Kind::File {
            doc.files += 1;
            language = language_of(path_of(raw));
        } else if let Some(path) = raw.strip_prefix("+++ b/") {
            language = language_of(path);
        } else if let Some(path) = raw.strip_prefix("+++ ") {
            language = language_of(path.trim_start_matches("b/"));
        }
        match kind {
            Kind::Add => doc.added = doc.added.saturating_add(1),
            Kind::Remove => doc.removed = doc.removed.saturating_add(1),
            _ => {}
        }
        let shown = clip(raw);
        let highlight_later = matches!(kind, Kind::Add | Kind::Remove | Kind::Context);
        doc.lines.push(Line {
            kind,
            text: shown,
            language: if highlight_later {
                language.clone()
            } else {
                String::new()
            },
            spans: if highlight_later {
                None
            } else {
                Some(Vec::new())
            },
        });
    }
    if cut {
        doc.lines.push(Line {
            kind: Kind::Meta,
            text: "The rest of the changes are not shown.".into(),
            language: String::new(),
            spans: Some(Vec::new()),
        });
    }
    doc
}

/// The unified diff at the end of `text`, when `text` contains one.
#[must_use]
pub fn extract(text: &str) -> Option<&str> {
    let start = text
        .rfind("\ndiff --git ")
        .map(|index| index + 1)
        .or_else(|| text.starts_with("diff --git ").then_some(0))?;
    Some(&text[start..])
}

fn classify(line: &str) -> Kind {
    if line.starts_with("diff --git ") {
        Kind::File
    } else if line.starts_with("@@") {
        Kind::Hunk
    } else if line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("index ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("rename ")
        || line.starts_with("similarity ")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
        || line.starts_with("Binary files")
        || line.starts_with('\\')
    {
        Kind::Meta
    } else if line.starts_with('+') {
        Kind::Add
    } else if line.starts_with('-') {
        Kind::Remove
    } else if line.starts_with(' ') {
        Kind::Context
    } else {
        Kind::Meta
    }
}

fn path_of(header: &str) -> &str {
    header
        .rsplit_once(" b/")
        .map(|(_, path)| path.trim().trim_matches('"'))
        .unwrap_or("")
}

fn language_of(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_owned())
        .unwrap_or_default()
}

fn clip(line: &str) -> String {
    if line.len() <= MAX_LINE {
        return line.to_owned();
    }
    let mut end = MAX_LINE;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let mut shown = line[..end].to_owned();
    shown.push('…');
    shown
}

fn highlight(line: &Line, highlighter: &Highlighter) -> Vec<Span> {
    if line.language.is_empty() || line.text.is_empty() {
        return Vec::new();
    }
    let prefix = usize::from(u8::from(matches!(
        line.text.as_bytes().first(),
        Some(b'+' | b'-' | b' ')
    )));
    if prefix >= line.text.len() {
        return Vec::new();
    }
    let code = &line.text[prefix..];
    let mut spans = highlighter.spans(&line.language, code);
    for span in &mut spans {
        span.start = span.start.saturating_add(prefix).min(line.text.len());
        span.end = span.end.saturating_add(prefix).min(line.text.len());
    }
    spans.retain(|span| span.end > span.start);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn sample(lines: usize) -> String {
        let mut diff = String::from(
            "diff --git a/src/answer.rs b/src/answer.rs\n--- a/src/answer.rs\n+++ b/src/answer.rs\n@@ -1,1 +1,4996 @@\n fn keep() {}\n",
        );
        let body = lines.saturating_sub(5);
        for index in 0..body {
            diff.push_str(&format!("+fn line_{index}() {{ return {index}; }}\n"));
        }
        diff
    }

    #[test]
    fn a_unified_diff_counts_files_and_changed_lines() {
        let doc = parse(
            "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1,2 @@\n-fn old() {}\n+fn answer() { return 1; }\n+fn extra() {}\n\ndiff --git a/notes.md b/notes.md\n--- a/notes.md\n+++ b/notes.md\n@@ -1 +0,0 @@\n-gone\n",
        );
        assert_eq!(doc.files, 2);
        assert_eq!((doc.added, doc.removed), (2, 2));
        assert!(doc.lines().iter().any(|line| line.kind == Kind::Hunk));
        assert_eq!(doc.summary(), "2 files, +2, −2");
        assert!(doc.lines().iter().all(|line| match line.kind {
            Kind::Add | Kind::Remove | Kind::Context => line.spans.is_none(),
            _ => line.spans.as_ref().is_some_and(Vec::is_empty),
        }));
    }

    #[test]
    fn syntax_spans_color_a_line_without_changing_it() {
        let mut doc = parse(
            "diff --git a/src/answer.rs b/src/answer.rs\n+++ b/src/answer.rs\n@@ -0,0 +1 @@\n+fn answer() { return 1; }\n",
        );
        let before: Vec<_> = doc.lines().iter().map(|line| line.text.clone()).collect();
        doc.ensure_spans(0, doc.len(), &Highlighter::default());
        let add = doc
            .lines()
            .iter()
            .find(|line| line.kind == Kind::Add)
            .expect("added line");
        assert_eq!(
            doc.lines()
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            before.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let spans = add.spans.as_ref().expect("spans");
        assert!(spans.iter().any(|span| span.end > span.start));
        assert!(spans.iter().all(|span| span.end <= add.text.len()));
    }

    #[test]
    fn a_five_thousand_line_diff_windows_without_walking_every_line() {
        let doc = parse(&sample(5_000));
        assert_eq!(doc.len(), 5_000);
        assert_eq!(doc.summary(), "1 file, +4995, −0");
        let (first, count) = doc.window(0.0, 400.0, 18.0);
        assert_eq!(first, 0);
        assert!(count <= WINDOW_CAP);
        assert!(count < 40);
        let started = Instant::now();
        let mut seen = 0usize;
        for step in 0..200 {
            let (next, shown) = doc.window(step as f32 * 18.0, 400.0, 18.0);
            assert!(shown <= WINDOW_CAP);
            assert!(shown < doc.len());
            seen = next;
        }
        assert!(seen > 100);
        assert!(started.elapsed().as_millis() < 50);
        let end = doc.scroll_limit(400.0, 18.0);
        let (first, count) = doc.window(end, 400.0, 18.0);
        assert_eq!(first + count, doc.len());
        assert!(doc.lines()[first + count - 1].text.contains("line_"));
    }

    #[test]
    fn extract_keeps_the_last_unified_diff_in_a_result() {
        let text = "Coder finished.\ndiff --git a/old b/old\n+old\n\nAnd then:\ndiff --git a/src/a.rs b/src/a.rs\n+fn answer() {}\n";
        let found = extract(text).expect("diff");
        assert!(found.starts_with("diff --git a/src/a.rs"));
        assert_eq!(parse(found).added, 1);
        assert!(extract("no changes here").is_none());
    }

    #[test]
    fn a_line_past_the_bound_is_cut_on_a_character_boundary() {
        let long = format!("+{}\n", "é".repeat(MAX_LINE));
        let diff = format!("diff --git a/a.txt b/a.txt\n{long}");
        let doc = parse(&diff);
        let line = doc
            .lines()
            .iter()
            .find(|line| line.kind == Kind::Add)
            .unwrap();
        assert!(line.text.ends_with('…'));
        assert!(line.text.is_char_boundary(line.text.len()));
        assert!(line.text.len() <= MAX_LINE + '…'.len_utf8());
    }
}
