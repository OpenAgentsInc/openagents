//! The Markdown a slide's body may hold, laid out onto a grid.
//!
//! The Coder repository's deck lays bodies out through its component core's
//! full Markdown renderer. A slide needs far less, so this module carries
//! the part a deck uses and nothing else: paragraphs, bulleted and numbered
//! lists, and three inline marks.
//!
//! - A blank line separates two blocks.
//! - A line that opens with `- ` or `* ` starts a bulleted item, and one
//!   that opens with a number and `. ` starts a numbered item. Any other
//!   line continues the block above it.
//! - `**bold**` draws bold and `*italic*` in italics; `` `code` `` drops
//!   its backticks. Every word draws at the base intensity the caller
//!   passes, so a body never competes with the slide's one full-intensity
//!   element; weight, not brightness, marks a list item's lead.
//!
//! Text wraps at word boundaries; a word longer than the width breaks where
//! it must. A list's marker sits at half intensity and its continuation
//! lines hang under the item's first word.

use crate::grid::{Grid, Style};
use coder_ui::theme::Intensity;

/// One word, or one piece of a word, in one style.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Word {
    text: String,
    style: Style,
    /// Whether a space separates this word from the one before it.
    spaced: bool,
}

/// One block of a body.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Paragraph(String),
    Item { marker: String, text: String },
}

/// How a body lays out: the style of plain text, and whether list items
/// keep a blank row between them.
#[derive(Clone, Copy, Debug)]
pub struct Prose {
    pub base: Style,
    pub spaced_items: bool,
}

impl Default for Prose {
    fn default() -> Prose {
        Prose {
            base: Style::at(Intensity::ThreeQuarters),
            spaced_items: true,
        }
    }
}

impl Prose {
    /// `source` laid out at `width` cells.
    pub fn layout(&self, source: &str, width: usize) -> Grid {
        let mut grid = Grid::new(width, 0);
        let mut row = 0;
        let mut previous: Option<&Part> = None;
        let parts = parts(source);
        for part in &parts {
            if let Some(before) = previous {
                let both_items =
                    matches!(before, Part::Item { .. }) && matches!(part, Part::Item { .. });
                if !both_items || self.spaced_items {
                    row += 1;
                }
            }
            row = match part {
                Part::Paragraph(text) => self.lines(&mut grid, row, 0, text, width),
                Part::Item { marker, text } => {
                    let hang = marker.chars().count() + 1;
                    grid.put_str(0, row, marker, Style::at(Intensity::Half));
                    self.lines(&mut grid, row, hang, text, width)
                }
            };
            previous = Some(part);
        }
        grid.truncate(row);
        grid
    }

    /// Wraps `text` from `row` with every line starting at `indent`, and
    /// returns the row after the last one written.
    fn lines(&self, grid: &mut Grid, row: usize, indent: usize, text: &str, width: usize) -> usize {
        let room = width.saturating_sub(indent).max(1);
        let lines = wrap(&words(text, self.base), room);
        for (offset, line) in lines.iter().enumerate() {
            let mut col = indent;
            for word in line {
                col += grid.put_str(col, row + offset, &word.text, word.style);
            }
            grid.grow(row + offset + 1);
        }
        row + lines.len().max(1)
    }
}

/// The blocks of `source`.
fn parts(source: &str) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    let mut open = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            open = false;
            continue;
        }
        if let Some((marker, rest)) = item(trimmed) {
            parts.push(Part::Item {
                marker,
                text: rest.to_string(),
            });
            open = true;
            continue;
        }
        match parts.last_mut() {
            Some(Part::Paragraph(text)) | Some(Part::Item { text, .. }) if open => {
                text.push(' ');
                text.push_str(trimmed);
            }
            _ => {
                parts.push(Part::Paragraph(trimmed.to_string()));
                open = true;
            }
        }
    }
    parts
}

/// The marker a list item draws and the rest of its line, when `line`
/// opens an item.
fn item(line: &str) -> Option<(String, &str)> {
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return Some(("•".to_string(), rest.trim_start()));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0
        && let Some(rest) = line[digits..].strip_prefix(". ")
    {
        return Some((line[..digits + 1].to_string(), rest.trim_start()));
    }
    None
}

/// The words of `text`, each carrying the style its inline marks give it.
fn words(text: &str, base: Style) -> Vec<Word> {
    let mut words = Vec::new();
    let (mut bold, mut italic, mut code) = (false, false, false);
    let mut current = String::new();
    let mut spaced = false;
    let mut pending_space = false;
    let style = |bold: bool, italic: bool, code: bool| {
        if bold {
            base.bold(true)
        } else if code {
            base
        } else {
            base.italic(italic)
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let flush = |words: &mut Vec<Word>, current: &mut String, style: Style, spaced: bool| {
        if !current.is_empty() {
            words.push(Word {
                text: std::mem::take(current),
                style,
                spaced,
            });
        }
    };
    while index < chars.len() {
        let c = chars[index];
        let here = style(bold, italic, code);
        if c == '`' {
            flush(&mut words, &mut current, here, spaced);
            spaced = false;
            code = !code;
            index += 1;
            continue;
        }
        if !code && c == '*' && chars.get(index + 1) == Some(&'*') {
            flush(&mut words, &mut current, here, spaced);
            spaced = false;
            bold = !bold;
            index += 2;
            continue;
        }
        if !code && c == '*' {
            flush(&mut words, &mut current, here, spaced);
            spaced = false;
            italic = !italic;
            index += 1;
            continue;
        }
        if c == ' ' {
            flush(&mut words, &mut current, here, spaced);
            pending_space = true;
            index += 1;
            continue;
        }
        if current.is_empty() {
            spaced = pending_space;
            pending_space = false;
        }
        current.push(c);
        index += 1;
    }
    flush(&mut words, &mut current, style(bold, italic, code), spaced);
    words
}

/// Greedy wrapping of `words` into lines of at most `width` cells. A word
/// joined to the one before it (no space between) moves with it.
fn wrap(words: &[Word], width: usize) -> Vec<Vec<Word>> {
    // Group the pieces that touch into the units a line break may not split.
    let mut units: Vec<Vec<Word>> = Vec::new();
    for word in words {
        match units.last_mut() {
            Some(unit) if !word.spaced => unit.push(word.clone()),
            _ => units.push(vec![word.clone()]),
        }
    }
    let length = |unit: &[Word]| unit.iter().map(|w| w.text.chars().count()).sum::<usize>();
    let mut lines: Vec<Vec<Word>> = Vec::new();
    let mut line: Vec<Word> = Vec::new();
    let mut used = 0;
    for unit in units {
        let size = length(&unit);
        let gap = usize::from(!line.is_empty());
        if used + gap + size > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        if size > width {
            // A unit longer than the line breaks where it must.
            for word in unit {
                for c in word.text.chars() {
                    if used >= width {
                        lines.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    push_char(&mut line, c, word.style);
                    used += 1;
                }
            }
            continue;
        }
        if !line.is_empty() {
            push_char(&mut line, ' ', unit[0].style);
            used += 1;
        }
        for word in unit {
            used += word.text.chars().count();
            line.push(Word {
                spaced: false,
                ..word
            });
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Appends `c` in `style` to the last piece of `line` when the styles
/// match, or as a new piece.
fn push_char(line: &mut Vec<Word>, c: char, style: Style) {
    match line.last_mut() {
        Some(last) if last.style == style => last.text.push(c),
        _ => line.push(Word {
            text: c.to_string(),
            style,
            spaced: false,
        }),
    }
}

/// `text` as one paragraph in `style`, wrapped at `width`.
pub fn text(text: &str, style: Style, width: usize) -> Grid {
    let sentence = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Prose {
        base: style,
        spaced_items: false,
    }
    .layout(&sentence, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bulleted list draws its marker at half, its lead bold at the
    /// base intensity, and hangs its continuation under the first word.
    #[test]
    fn a_list_item_hangs_and_marks_its_lead() {
        let grid = Prose::default().layout(
            "- **The lead.** and a continuation that is long enough to wrap",
            24,
        );
        let text = grid.to_text();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("• The lead."), "{text}");
        assert!(lines[1].starts_with("  "), "{text}");
        assert_eq!(grid.get(0, 0).unwrap().style.intensity, Intensity::Half);
        let lead = grid.get(2, 0).unwrap().style;
        assert!(lead.bold);
        assert_eq!(lead.intensity, Intensity::ThreeQuarters);
        let rest = grid.get(14, 0).unwrap().style;
        assert_eq!(rest.intensity, Intensity::ThreeQuarters);
    }

    /// Items keep a blank row between them when asked, and paragraphs
    /// always do.
    #[test]
    fn blocks_are_separated() {
        let spaced = Prose::default().layout("- one\n- two\n\nafter", 20);
        assert_eq!(spaced.to_text(), "• one\n\n• two\n\nafter\n");
        let tight = Prose {
            spaced_items: false,
            ..Prose::default()
        }
        .layout("1. one\n2. two", 20);
        assert_eq!(tight.to_text(), "1. one\n2. two\n");
    }

    /// Nothing sets wider than the width, and a word longer than the line
    /// breaks where it must.
    #[test]
    fn nothing_sets_past_the_width() {
        let grid = text(
            "a sentence with averyveryverylongwordindeed in it",
            Style::at(Intensity::Full),
            12,
        );
        for line in grid.to_text().lines() {
            assert!(line.chars().count() <= 12, "{line}");
        }
    }

    /// Punctuation touching a mark stays with its word across a break.
    #[test]
    fn punctuation_stays_with_its_word() {
        let grid = Prose::default().layout("aaaa bbbb **cccc**.", 10);
        assert_eq!(grid.to_text(), "aaaa bbbb\ncccc.\n");
    }
}
