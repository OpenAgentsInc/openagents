//! The views over the chat, and the mouse.
//!
//! - The run view (Ctrl+R, `/run`) fills the screen with the thread's one
//!   Coder run: every step, each command with its output, and the result
//!   with its changes. Its composer sends the run what you type (steering):
//!   the run reads it at its next step.
//! - The file view opens when you click a file's path in a reply or a run:
//!   the file, read only, with line numbers and the code highlighted.
//! - Dragging the mouse selects text on the screen, and letting go copies
//!   it. A click that does not drag opens the file whose path it is on.
//!
//! What the last frame drew ([`Shown`]) is what a drag selects and a click
//! reads, so a selection is exactly the text on the screen.

use std::path::{Path, PathBuf};

use code_highlight::grok::{self, syntect::easy::HighlightLines};
use coder_terminal::markdown;
use coder_terminal::{Intensity, Ladder};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The most bytes of a file the file view reads.
pub const FILE_MAX: u64 = 2 * 1024 * 1024;

/// The run view: the thread's Coder run over the whole screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunView {
    /// Rows scrolled back from the bottom.
    pub scroll: usize,
}

/// A file, read only.
#[derive(Clone, Debug, PartialEq)]
pub struct FileView {
    /// The path as it was clicked, which the title shows.
    pub path: String,
    /// Each line, numbered and highlighted.
    pub lines: Vec<Line<'static>>,
    /// The first line shown.
    pub top: usize,
}

impl FileView {
    /// Scroll by `rows`, up when negative, staying within the file.
    pub fn scroll(&mut self, rows: isize, height: usize) {
        let last = self.lines.len().saturating_sub(height.max(1));
        self.top = self.top.saturating_add_signed(rows).min(last);
    }
}

/// A stretch of the screen the mouse is selecting: from where the button
/// went down to where it is now, in screen cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (u16, u16),
    pub head: (u16, u16),
}

impl Selection {
    /// Its two ends, the earlier on the screen first.
    pub fn ends(&self) -> ((u16, u16), (u16, u16)) {
        let key = |(x, y): (u16, u16)| (y, x);
        if key(self.anchor) <= key(self.head) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Whether the cell at `x`, `y` is selected.
    pub fn covers(&self, x: u16, y: u16) -> bool {
        let ((x0, y0), (x1, y1)) = self.ends();
        (y0..=y1).contains(&y) && (y > y0 || x >= x0) && (y < y1 || x <= x1)
    }

    /// Whether the button came up where it went down: a click.
    pub fn is_click(&self) -> bool {
        self.anchor == self.head
    }
}

/// What the last frame drew where text can be selected: each row's cells.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub area: Rect,
    pub rows: Vec<Vec<String>>,
}

impl Shown {
    /// The cells of `area` as `buf` holds them.
    pub fn capture(buf: &Buffer, area: Rect) -> Self {
        let rows = (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buf[(x, y)].symbol().to_owned())
                    .collect()
            })
            .collect();
        Shown { area, rows }
    }

    /// Whether `x`, `y` is in it.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.area.left()
            && x < self.area.right()
            && y >= self.area.top()
            && y < self.area.bottom()
    }

    /// The selected text: whole rows between the ends, each without its
    /// trailing blanks, one line each.
    pub fn text(&self, selection: &Selection) -> String {
        let ((x0, y0), (x1, y1)) = selection.ends();
        let mut out = Vec::new();
        for y in y0.max(self.area.top())..=y1.min(self.area.bottom().saturating_sub(1)) {
            let Some(row) = self.rows.get(usize::from(y - self.area.top())) else {
                continue;
            };
            let from = if y == y0 { x0 } else { self.area.left() };
            let to = if y == y1 {
                x1
            } else {
                self.area.right().saturating_sub(1)
            };
            let from = usize::from(from.saturating_sub(self.area.left()));
            let to =
                usize::from(to.saturating_sub(self.area.left())).min(row.len().saturating_sub(1));
            let line: String = row
                .get(from..=to)
                .map(|cells| cells.concat())
                .unwrap_or_default();
            out.push(line.trim_end().to_owned());
        }
        out.join("\n")
    }

    /// The run of non-blank text the cell at `x`, `y` is in.
    pub fn word(&self, x: u16, y: u16) -> Option<String> {
        if !self.contains(x, y) {
            return None;
        }
        let row = self.rows.get(usize::from(y - self.area.top()))?;
        let at = usize::from(x - self.area.left());
        let blank = |cell: &String| cell.trim().is_empty() && !cell.is_empty();
        if row.get(at).is_none_or(blank) {
            return None;
        }
        let start = row[..at].iter().rposition(blank).map_or(0, |i| i + 1);
        let end = row[at..]
            .iter()
            .position(blank)
            .map_or(row.len(), |i| at + i);
        Some(row[start..end].concat())
    }
}

/// The file a word on the screen names, and the line it points at:
/// `src/app.rs`, `src/app.rs:42`, `src/app.rs:42:7`, or one quoted or
/// bracketed. A bounded parse of a word the person clicked; whether it is
/// a file at all is the file system's to say ([`open`]).
pub fn reference(word: &str) -> Option<(String, Option<usize>)> {
    let bracket = |c: char| {
        matches!(
            c,
            '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | '{' | '}' | ',' | ';'
        )
    };
    let word = word
        .trim_start_matches(bracket)
        .trim_end_matches(|c: char| bracket(c) || c == '.' || c == ':');
    if word.is_empty() || word.contains("://") {
        return None;
    }
    let mut parts = word.splitn(3, ':');
    let path = parts.next()?.to_owned();
    let line = parts
        .next()
        .and_then(|line| line.parse::<usize>().ok())
        .filter(|line| *line > 0);
    // A Windows drive letter (`C:\x`) is part of the path.
    let path = if path.len() == 1 && word.len() > 2 && word.as_bytes()[1] == b':' {
        word.to_owned()
    } else {
        path
    };
    (!path.is_empty()).then_some((path, line))
}

/// Opens `path` read only: an absolute path as it is, a relative one under
/// the first of `bases` that holds it. `None` when no such file is here
/// (the word clicked was not a file's path).
///
/// # Errors
/// Too large, not text, or unreadable.
pub fn open(
    path: &str,
    line: Option<usize>,
    bases: &[PathBuf],
    ladder: Ladder,
) -> Result<Option<FileView>, String> {
    let found = if Path::new(path).is_absolute() {
        Some(PathBuf::from(path))
    } else {
        bases
            .iter()
            .map(|base| base.join(path))
            .find(|candidate| candidate.is_file())
    }
    .filter(|candidate| candidate.is_file());
    let Some(found) = found else {
        return Ok(None);
    };
    let size = std::fs::metadata(&found).map_err(|e| e.to_string())?.len();
    if size > FILE_MAX {
        return Err(format!("{path} is over 2 MB; open it in an editor."));
    }
    let bytes = std::fs::read(&found).map_err(|e| format!("Cannot read {path}: {e}"))?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err(format!("{path} is not a text file."));
    }
    let text = String::from_utf8_lossy(&bytes);
    let language = found.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let lines = file_lines(&text, language, ladder);
    let top = line.map_or(0, |line| line.saturating_sub(4));
    let mut view = FileView {
        path: path.to_owned(),
        lines,
        top: 0,
    };
    view.scroll(isize::try_from(top).unwrap_or(isize::MAX), 1);
    Ok(Some(view))
}

/// A file's lines: a line number on the quarter step, then the code
/// highlighted as grok-build highlights a file (syntect by the file's
/// extension, the palette's theme at the terminal's color level), as a code
/// block draws it. A file over `code_highlight::MAX_BYTES` draws plain.
pub fn file_lines(text: &str, language: &str, ladder: Ladder) -> Vec<Line<'static>> {
    let syntect = markdown::palette().syntect();
    let level = markdown::syntax_level(ladder);
    let mut highlighter = (text.len() <= code_highlight::MAX_BYTES)
        .then(|| syntect.highlight_lines_by_file_path(&Path::new("file").with_extension(language)))
        .flatten();
    let count = text.lines().count().max(1);
    let gutter = count.to_string().len();
    let number = ladder.style(Intensity::Quarter);
    let plain = ladder.style(Intensity::Full);
    let mut out = Vec::with_capacity(count);
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let line = line.trim_end_matches(['\n', '\r']).replace('\t', "    ");
        let mut spans = vec![Span::styled(format!("{:>gutter$}  ", index + 1), number)];
        spans.extend(highlight_line(
            &line,
            &mut highlighter,
            syntect,
            level,
            plain,
        ));
        out.push(Line::from(spans));
    }
    out
}

/// grok-build's `highlight_line`: syntect's segments as styled spans, or
/// the whole line in `fallback` when there is no highlighter or it fails.
fn highlight_line(
    text: &str,
    highlighter: &mut Option<HighlightLines<'_>>,
    syntect: &grok::Syntect,
    level: grok::ColorLevel,
    fallback: Style,
) -> Vec<Span<'static>> {
    if let Some(hl) = highlighter.as_mut()
        && let Ok(ranges) = hl.highlight_line(&format!("{text}\n"), &syntect.syntax_set)
    {
        let mut spans = Vec::new();
        for (style, segment) in ranges {
            let mut s = segment.to_owned();
            while s.ends_with('\n') || s.ends_with('\r') {
                s.pop();
            }
            if s.is_empty() {
                continue;
            }
            spans.push(Span::styled(
                s,
                grok::color::syntect_to_ratatui_fg(style, level),
            ));
        }
        if !spans.is_empty() {
            return spans;
        }
    }
    if text.is_empty() {
        return Vec::new();
    }
    vec![Span::styled(text.to_string(), fallback)]
}

/// Paints `selection` over the cells of `area` it covers: the ladder's
/// selection tint, or reversed where the terminal has no color.
pub fn paint(buf: &mut Buffer, area: Rect, selection: &Selection, ladder: Ladder) {
    let tint = ladder.selection();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if selection.covers(x, y) {
                let cell = &mut buf[(x, y)];
                if tint == ratatui::style::Color::Reset {
                    cell.modifier.insert(Modifier::REVERSED);
                } else {
                    cell.bg = tint;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_is_a_path_with_perhaps_a_line() {
        assert_eq!(reference("src/app.rs"), Some(("src/app.rs".into(), None)));
        assert_eq!(
            reference("`crates/a/src/lib.rs:42:7`,"),
            Some(("crates/a/src/lib.rs".into(), Some(42)))
        );
        assert_eq!(reference("(README.md)."), Some(("README.md".into(), None)));
        assert_eq!(reference("https://example.com/a.rs"), None);
        assert_eq!(reference("``"), None);
    }

    fn shown(rows: &[&str]) -> Shown {
        Shown {
            area: Rect::new(2, 1, 20, rows.len() as u16),
            rows: rows
                .iter()
                .map(|row| format!("{row:<20}").chars().map(String::from).collect())
                .collect(),
        }
    }

    #[test]
    fn a_drag_selects_the_text_between_its_ends() {
        let screen = shown(&["one two three", "four five", "six"]);
        // From "two" on the first row to "fi" on the second, dragged
        // backwards: the ends sort.
        let selection = Selection {
            anchor: (2 + 5, 2),
            head: (2 + 4, 1),
        };
        assert_eq!(screen.text(&selection), "two three\nfour f");
        assert!(selection.covers(2 + 10, 1) && !selection.covers(2 + 3, 1));
        assert_eq!(screen.word(2 + 7, 2), Some("five".into()));
        assert_eq!(screen.word(2 + 4, 2), None, "a blank");
        assert_eq!(screen.word(40, 2), None, "outside");
    }

    #[test]
    fn a_file_opens_numbered_highlighted_and_at_its_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let body: String = (1..=40).map(|n| format!("fn f{n}() {{}}\n")).collect();
        std::fs::write(dir.path().join("src/lib.rs"), &body).unwrap();
        std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2]).unwrap();
        let ladder = Ladder::default();
        let bases = [dir.path().join("missing"), dir.path().to_path_buf()];
        let view = open("src/lib.rs", Some(30), &bases, ladder)
            .unwrap()
            .unwrap();
        assert_eq!(view.lines.len(), 40);
        assert_eq!(view.top, 26, "a few lines above the one named");
        let first: String = view.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(first, " 1  fn f1() {}");
        // `fn` is a keyword: Grok Night's keyword color, as code blocks
        // draw it, not the plain text around it.
        let keyword = view.lines[0]
            .spans
            .iter()
            .find(|s| s.content == "fn")
            .unwrap();
        let name = view.lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("f1"))
            .unwrap();
        assert!(matches!(
            keyword.style.fg,
            Some(ratatui::style::Color::Rgb(..))
        ));
        assert_ne!(keyword.style.fg, name.style.fg);
        assert!(
            open("blob.bin", None, &bases, ladder)
                .unwrap_err()
                .contains("not a text file")
        );
        assert_eq!(open("nope.rs", None, &bases, ladder), Ok(None));
        assert_eq!(open("src", None, &bases, ladder), Ok(None), "a folder");
    }
}
