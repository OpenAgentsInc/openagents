//! Text selection and search over a pane's scrollback and screen.
//!
//! A [`Point`] names a cell by its line's absolute number, which stays the
//! same while output scrolls the line up and into the scrollback, so a
//! selection follows its text. Selections grow by characters, words
//! (double-click), or whole lines (triple-click), and [`Selection::text`]
//! reads them back the way a terminal copies: wide characters once,
//! combining marks kept, wrapped lines joined, and other lines ended with
//! a newline and trimmed of trailing blanks.

use coder_vt::{Row, Terminal};

/// A cell: its line's absolute number and its column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    pub line: u64,
    pub col: usize,
}

/// What a selection grows by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Char,
    Word,
    Line,
}

/// A selection from where it started to where it reaches now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: Point,
    pub head: Point,
    pub unit: Unit,
}

/// The absolute number of `index` in [`Terminal::line`].
#[must_use]
pub fn absolute(vt: &Terminal, index: usize) -> u64 {
    vt.history_dropped() + index as u64
}

/// The [`Terminal::line`] index of absolute line `line`, while it is kept.
#[must_use]
pub fn index(vt: &Terminal, line: u64) -> Option<usize> {
    let index = usize::try_from(line.checked_sub(vt.history_dropped())?).ok()?;
    (index < total(vt)).then_some(index)
}

/// Lines in the scrollback and on the screen.
#[must_use]
pub fn total(vt: &Terminal) -> usize {
    vt.scrollback_len() + vt.rows()
}

/// The [`Terminal::line`] index of the top visible row, `scroll` lines
/// back from the bottom.
#[must_use]
pub fn top(vt: &Terminal, scroll: usize) -> usize {
    vt.scrollback_len() - scroll.min(vt.scrollback_len())
}

fn row_at(vt: &Terminal, line: u64) -> Option<&Row> {
    vt.line(index(vt, line)?)
}

/// Whether `c` is part of a word for a double-click: anything but blanks
/// and the brackets, quotes, and separators that end words in commands.
#[must_use]
pub fn word_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '"'
                | '\''
                | '`'
                | ','
                | ';'
                | '|'
                | '│'
        )
}

impl Selection {
    /// A selection that starts and ends at `point`.
    #[must_use]
    pub fn at(point: Point, unit: Unit) -> Self {
        Selection {
            anchor: point,
            head: point,
            unit,
        }
    }

    /// Whether a character selection covers nothing yet.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.unit == Unit::Char && self.anchor == self.head
    }

    /// The first and last selected cells, in order and grown to whole
    /// words or lines.
    #[must_use]
    pub fn range(&self, vt: &Terminal) -> (Point, Point) {
        let (mut start, mut end) = if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        match self.unit {
            Unit::Char => {}
            Unit::Word => {
                start = word_start(vt, start);
                end = word_end(vt, end);
            }
            Unit::Line => {
                start = Point {
                    line: logical_first(vt, start.line),
                    col: 0,
                };
                let last = logical_last(vt, end.line);
                end = Point {
                    line: last,
                    col: row_at(vt, last).map_or(0, |r| r.cells.len().saturating_sub(1)),
                };
            }
        }
        (start, end)
    }

    /// The selected columns of absolute line `line`, as `from..to`, or
    /// `None` when the line has none.
    #[must_use]
    pub fn columns(&self, vt: &Terminal, line: u64, cols: usize) -> Option<(usize, usize)> {
        let (start, end) = self.range(vt);
        if line < start.line || line > end.line {
            return None;
        }
        let from = if line == start.line { start.col } else { 0 };
        let to = if line == end.line { end.col + 1 } else { cols };
        (from < to.min(cols)).then_some((from, to.min(cols)))
    }

    /// The selected text.
    #[must_use]
    pub fn text(&self, vt: &Terminal) -> String {
        let (start, end) = self.range(vt);
        let mut out = String::new();
        let mut line = start.line;
        while line <= end.line {
            let Some(row) = row_at(vt, line) else {
                line += 1;
                continue;
            };
            let from = if line == start.line { start.col } else { 0 };
            let to = if line == end.line {
                end.col + 1
            } else {
                row.cells.len()
            };
            let joined = row.wrapped && line != end.line;
            out.push_str(&cells_text(row, from, to, !joined));
            if !joined && line != end.line {
                out.push('\n');
            }
            line += 1;
        }
        out
    }
}

/// The text of `row`'s columns `from..to`: a wide character once, even
/// when only its right half is in range, and combining marks kept.
#[must_use]
pub fn cells_text(row: &Row, from: usize, to: usize, trim: bool) -> String {
    let to = to.min(row.cells.len());
    let mut col = from.min(to);
    if col > 0 && col < to && row.cells[col].width == 0 {
        col -= 1;
    }
    let mut text = String::new();
    for cell in &row.cells[col..to] {
        if cell.width == 0 {
            continue;
        }
        text.push(cell.ch);
        text.extend(cell.combining.iter());
    }
    if trim {
        text.truncate(text.trim_end_matches(' ').len());
    }
    text
}

fn cell_char(row: &Row, col: usize) -> Option<char> {
    let mut col = col.min(row.cells.len().checked_sub(1)?);
    if col > 0 && row.cells[col].width == 0 {
        col -= 1;
    }
    Some(row.cells[col].ch)
}

fn word_start(vt: &Terminal, point: Point) -> Point {
    let Some(row) = row_at(vt, point.line) else {
        return point;
    };
    if !cell_char(row, point.col).is_some_and(word_char) {
        return point;
    }
    let mut col = point.col.min(row.cells.len().saturating_sub(1));
    while col > 0 && cell_char(row, col - 1).is_some_and(word_char) {
        col -= 1;
    }
    Point {
        line: point.line,
        col,
    }
}

fn word_end(vt: &Terminal, point: Point) -> Point {
    let Some(row) = row_at(vt, point.line) else {
        return point;
    };
    if !cell_char(row, point.col).is_some_and(word_char) {
        return point;
    }
    let mut col = point.col;
    while col + 1 < row.cells.len() && cell_char(row, col + 1).is_some_and(word_char) {
        col += 1;
    }
    Point {
        line: point.line,
        col,
    }
}

/// The first row of the logical line `line` is part of.
fn logical_first(vt: &Terminal, mut line: u64) -> u64 {
    while line > 0 && row_at(vt, line - 1).is_some_and(|r| r.wrapped) {
        line -= 1;
    }
    line
}

/// The last row of the logical line `line` is part of.
fn logical_last(vt: &Terminal, mut line: u64) -> u64 {
    while row_at(vt, line).is_some_and(|r| r.wrapped) && row_at(vt, line + 1).is_some() {
        line += 1;
    }
    line
}

/// Finds `query` searching toward older lines from just before `from`
/// (`older`), or toward newer lines from just after it. A query in
/// lowercase matches either case. Returns the match's first and last
/// cells.
#[must_use]
pub fn search(vt: &Terminal, query: &str, from: Point, older: bool) -> Option<(Point, Point)> {
    if query.is_empty() {
        return None;
    }
    let fold = !query.chars().any(char::is_uppercase);
    let needle: Vec<char> = if fold {
        query.chars().flat_map(char::to_lowercase).collect()
    } else {
        query.chars().collect()
    };
    let first = vt.history_dropped();
    let last = first + total(vt) as u64 - 1;
    let mut line = from.line.clamp(first, last);
    loop {
        if let Some(row) = row_at(vt, line) {
            // Each drawn character and its column.
            let cells: Vec<(usize, char)> = row
                .cells
                .iter()
                .enumerate()
                .filter(|(_, c)| c.width != 0)
                .map(|(col, c)| {
                    let ch = if fold {
                        c.ch.to_lowercase().next().unwrap_or(c.ch)
                    } else {
                        c.ch
                    };
                    (col, ch)
                })
                .collect();
            let starts = (0..cells.len().saturating_sub(needle.len() - 1)).filter(|&i| {
                cells[i..i + needle.len()]
                    .iter()
                    .map(|(_, c)| *c)
                    .eq(needle.iter().copied())
            });
            let hit = if older {
                starts
                    .filter(|&i| line < from.line || cells[i].0 < from.col)
                    .last()
            } else {
                starts
                    .clone()
                    .find(|&i| line > from.line || cells[i].0 > from.col)
            };
            if let Some(i) = hit {
                let end = cells[i + needle.len() - 1];
                let end_col = end.0 + usize::from(row.cells[end.0].width.max(1)) - 1;
                return Some((
                    Point {
                        line,
                        col: cells[i].0,
                    },
                    Point { line, col: end_col },
                ));
            }
        }
        if older {
            if line == first {
                return None;
            }
            line -= 1;
        } else {
            if line == last {
                return None;
            }
            line += 1;
        }
    }
}
