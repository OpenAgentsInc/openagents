//! The composer: the line `oa coder` types into.
//!
//! The editing mechanics are ported from the ratatui textarea in grok-build
//! (Apache-2.0) — see `edit.rs` and `keys.rs` for the provenance and for what
//! was trimmed. This module is the part that is ours: the soft-wrap geometry
//! the composer draws itself with, vertical motion over those rows, and the
//! small dispatch that turns a key into an edit, a newline, or a submission.

pub mod complete;
pub mod edit;
pub mod history;
pub mod keys;

use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthStr as _;

use edit::EditBuffer;

/// Whether a slash-prefixed line belongs to the local command dispatcher.
///
/// Known commands always stay local. An unknown, command-shaped token such as
/// `/difff` also stays local so the interface can report the typo. Text that
/// contains an argument, another path separator, or a filename marker is a
/// prompt instead. This keeps paths and ordinary prose from being swallowed by
/// the command interface.
pub fn is_local_slash_input(text: &str, commands: &[(&str, &str)]) -> bool {
    let trimmed = text.trim_start();
    let Some(body) = trimmed.strip_prefix('/') else {
        return false;
    };
    let Some(name) = body.split_whitespace().next() else {
        return true;
    };

    if commands.iter().any(|(command, _)| *command == name) {
        return true;
    }

    let command_shaped = !body.chars().any(char::is_whitespace)
        && !name.contains('/')
        && !name.contains('.')
        && !name.starts_with('~');
    command_shaped
}

/// What a key did to the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerAction {
    /// The key meant nothing here; the caller may still want it.
    Ignored,
    /// The caret moved and the text did not.
    ///
    /// Told apart from [`ComposerAction::Redraw`] because a caller walking the
    /// input history has to know whether the reader has started editing: a
    /// caret move continues the walk, and a change to the text ends it.
    Moved,
    /// The composer's text changed and the frame is stale.
    Redraw,
    /// Enter, with the text that was in the composer. The composer is now empty.
    Submit(String),
}

/// A multi-line input with a caret.
#[derive(Debug, Default)]
pub struct Composer {
    buffer: EditBuffer,
    /// The display column a vertical move is aiming for. Set on the first of a
    /// run of Up/Down presses and held across the run, so walking down through
    /// a short line and out the other side returns to the column you started
    /// in rather than to the short line's end.
    preferred_col: Option<usize>,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn cursor_byte(&self) -> usize {
        self.buffer.cursor_byte()
    }

    /// Empty the composer and return what was typed.
    pub fn take(&mut self) -> String {
        self.preferred_col = None;
        self.buffer.take()
    }

    pub fn insert_str(&mut self, text: &str) {
        self.preferred_col = None;
        let _ = self.buffer.insert_str(text);
    }

    /// Replace everything in the composer, leaving the caret at the end.
    ///
    /// What walking the input history does. The caret goes to the end because
    /// a recalled prompt is nearly always being added to.
    pub fn set_text(&mut self, text: &str) {
        self.preferred_col = None;
        self.buffer = EditBuffer::from_text(text);
        let end = self.buffer.text().len();
        let _ = self.buffer.set_cursor_byte(end);
    }

    /// The rows the composer draws, soft-wrapped to `width` columns.
    pub fn rows(&self, width: usize) -> Vec<&str> {
        wrap_rows(self.text(), width)
            .into_iter()
            .map(|range| &self.text()[range])
            .collect()
    }

    /// Where the caret sits, as (row index, display column).
    pub fn cursor_rowcol(&self, width: usize) -> (usize, usize) {
        let rows = wrap_rows(self.text(), width);
        let cursor = self.buffer.cursor_byte();
        let index = row_of(&rows, cursor);
        let row = &rows[index];
        let column = self.text()[row.start..cursor.max(row.start).min(row.end)].width();
        (index, column)
    }

    /// Apply a key.
    ///
    /// Enter submits and Alt+Enter (or Ctrl+J, or Shift+Enter where the
    /// terminal reports it) inserts a newline. Everything else goes to the
    /// ported classifier, so the readline chords it knows — Ctrl+A, Ctrl+E,
    /// Ctrl+W, Ctrl+K, Ctrl+U, Alt+B, Alt+F — work here too.
    pub fn handle_key(&mut self, key: &KeyEvent, width: usize) -> ComposerAction {
        match key.code {
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert_str("\n");
                return ComposerAction::Redraw;
            }
            KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => {
                self.insert_str("\n");
                return ComposerAction::Redraw;
            }
            KeyCode::Enter => {
                let text = self.take();
                return ComposerAction::Submit(text);
            }
            KeyCode::Up => {
                return self.move_vertically(-1, width);
            }
            KeyCode::Down => {
                return self.move_vertically(1, width);
            }
            _ => {}
        }

        let Some(command) = keys::classify_key_event(key) else {
            return ComposerAction::Ignored;
        };
        // Any horizontal or editing command abandons the column a vertical run
        // was aiming for.
        self.preferred_col = None;
        // The key was the composer's even when the caret was already at the
        // edge it was asked to move to, so the caller does not also get it.
        let outcome = self.buffer.apply(command);
        if outcome.text_changed() {
            ComposerAction::Redraw
        } else {
            ComposerAction::Moved
        }
    }

    /// Move the caret one wrapped row up or down, holding the preferred column.
    fn move_vertically(&mut self, delta: isize, width: usize) -> ComposerAction {
        let rows = wrap_rows(self.text(), width);
        let cursor = self.buffer.cursor_byte();
        let index = row_of(&rows, cursor);
        let column = self
            .preferred_col
            .unwrap_or_else(|| self.text()[rows[index].start..cursor].width());

        let target = index as isize + delta;
        if target < 0 || target as usize >= rows.len() {
            // Off the end of the composer. The caller takes the key, which is
            // how Up on a one-line composer reaches the transcript.
            self.preferred_col = None;
            return ComposerAction::Ignored;
        }

        self.preferred_col = Some(column);
        let row = &rows[target as usize];
        let byte = byte_at_column(self.text(), row.clone(), column);
        let _ = self.buffer.set_cursor_byte(byte);
        ComposerAction::Moved
    }
}

/// The index of the row holding `cursor`.
///
/// Rows do not abut: the newline between two logical lines, and the spaces a
/// soft break consumes, are in no row at all. A caret in one of those gaps
/// belongs to the row that ends there, which is what puts it at the end of the
/// line you just walked up to rather than at the start of the next one.
///
/// The exception is a break with no gap — a word split mid-word because it was
/// longer than the row. There the caret belongs to the row that starts at that
/// byte, because that is where the next character will appear.
fn row_of(rows: &[Range<usize>], cursor: usize) -> usize {
    for (index, row) in rows.iter().enumerate() {
        if cursor < row.start {
            return index.saturating_sub(1);
        }
        if cursor < row.end {
            return index;
        }
        if cursor == row.end {
            let split_here = rows.get(index + 1).is_some_and(|next| next.start == cursor);
            if !split_here {
                return index;
            }
        }
    }
    rows.len().saturating_sub(1)
}

/// The byte offset within `row` closest to display column `column`.
fn byte_at_column(text: &str, row: Range<usize>, column: usize) -> usize {
    let mut byte = row.start;
    let mut seen = 0usize;
    for (offset, grapheme) in text[row.clone()].grapheme_indices(true) {
        let w = grapheme.width().max(1);
        if seen + w > column {
            return row.start + offset;
        }
        seen += w;
        byte = row.start + offset + grapheme.len();
    }
    byte
}

/// Soft-wrap `text` into byte ranges, one per display row.
///
/// Hard newlines always start a row. Within a logical line, a row breaks at
/// the last space that fits, and a word longer than the row breaks mid-word
/// rather than running off the edge. The spaces a break consumes belong to
/// neither row, so a wrapped paragraph has no leading space on its
/// continuations.
///
/// Every logical line contributes at least one range, and the ranges cover the
/// text in order, so a caret anywhere in the text — including at its very end
/// — falls in exactly one row.
pub fn wrap_rows(text: &str, width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut line_start = 0usize;

    loop {
        let line_end = text[line_start..]
            .find('\n')
            .map_or(text.len(), |offset| line_start + offset);
        wrap_one_line(text, line_start..line_end, width, &mut rows);
        if line_end == text.len() {
            break;
        }
        line_start = line_end + 1;
    }

    if rows.is_empty() {
        rows.push(0..0);
    }
    rows
}

fn wrap_one_line(text: &str, line: Range<usize>, width: usize, rows: &mut Vec<Range<usize>>) {
    let mut cursor = line.start;
    loop {
        let mut consumed = 0usize;
        let mut end = cursor;
        // The last point at which a break would land between words.
        let mut break_at: Option<usize> = None;
        let mut overflowed = false;

        for (offset, grapheme) in text[cursor..line.end].grapheme_indices(true) {
            let at = cursor + offset;
            let w = grapheme.width().max(1);
            if consumed + w > width {
                overflowed = true;
                break;
            }
            if grapheme == " " && at > cursor {
                break_at = Some(at);
            }
            consumed += w;
            end = at + grapheme.len();
        }

        if !overflowed {
            rows.push(cursor..line.end);
            return;
        }

        // Break at the space if there was one, otherwise mid-word at the last
        // grapheme that fit. A row that fits nothing at all still advances by
        // one grapheme, so this cannot spin.
        let (row_end, mut next) = match break_at {
            Some(space) => (space, space + 1),
            None if end > cursor => (end, end),
            None => {
                let one = text[cursor..line.end]
                    .grapheme_indices(true)
                    .next()
                    .map_or(line.end, |(_, g)| cursor + g.len());
                (one, one)
            }
        };
        rows.push(cursor..row_end);
        // A run of spaces at a break belongs to the break, not to the next row.
        while next < line.end && text[next..].starts_with(' ') {
            next += 1;
        }
        cursor = next;
        if cursor >= line.end {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(composer: &mut Composer, text: &str) {
        for ch in text.chars() {
            composer.handle_key(&key(KeyCode::Char(ch)), 40);
        }
    }

    #[test]
    fn typing_lands_in_the_buffer() {
        let mut c = Composer::new();
        typed(&mut c, "hello");
        assert_eq!(c.text(), "hello");
        assert_eq!(c.cursor_rowcol(40), (0, 5));
    }

    #[test]
    fn backspace_removes_the_last_character() {
        let mut c = Composer::new();
        typed(&mut c, "hello");
        c.handle_key(&key(KeyCode::Backspace), 40);
        assert_eq!(c.text(), "hell");
    }

    #[test]
    fn left_and_right_move_the_caret_without_changing_the_text() {
        let mut c = Composer::new();
        typed(&mut c, "abc");
        c.handle_key(&key(KeyCode::Left), 40);
        c.handle_key(&key(KeyCode::Left), 40);
        assert_eq!(c.cursor_rowcol(40), (0, 1));
        typed(&mut c, "X");
        assert_eq!(c.text(), "aXbc");
    }

    #[test]
    fn enter_submits_and_empties_the_composer() {
        let mut c = Composer::new();
        typed(&mut c, "ship it");
        let action = c.handle_key(&key(KeyCode::Enter), 40);
        assert_eq!(action, ComposerAction::Submit("ship it".to_string()));
        assert!(c.is_empty());
    }

    #[test]
    fn alt_enter_inserts_a_newline_instead_of_submitting() {
        let mut c = Composer::new();
        typed(&mut c, "one");
        let action = c.handle_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), 40);
        assert_eq!(action, ComposerAction::Redraw);
        typed(&mut c, "two");
        assert_eq!(c.text(), "one\ntwo");
        assert_eq!(c.cursor_rowcol(40), (1, 3));
    }

    #[test]
    fn esc_is_not_the_composers_key() {
        let mut c = Composer::new();
        assert_eq!(
            c.handle_key(&key(KeyCode::Esc), 40),
            ComposerAction::Ignored
        );
    }

    #[test]
    fn up_off_the_top_hands_the_key_back() {
        let mut c = Composer::new();
        typed(&mut c, "one line");
        assert_eq!(c.handle_key(&key(KeyCode::Up), 40), ComposerAction::Ignored);
    }

    #[test]
    fn up_and_down_hold_the_column_across_a_short_line() {
        let mut c = Composer::new();
        typed(&mut c, "abcdefgh");
        c.handle_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), 40);
        typed(&mut c, "ab");
        c.handle_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), 40);
        typed(&mut c, "abcdefgh");
        // Caret is at column 8 on row 2. Up lands on the short row's end, and
        // Up again returns to column 8.
        c.handle_key(&key(KeyCode::Up), 40);
        assert_eq!(c.cursor_rowcol(40), (1, 2));
        c.handle_key(&key(KeyCode::Up), 40);
        assert_eq!(c.cursor_rowcol(40), (0, 8));
    }

    #[test]
    fn wrap_breaks_at_a_space() {
        let text = "the quick brown fox";
        let rows: Vec<&str> = wrap_rows(text, 10).into_iter().map(|r| &text[r]).collect();
        assert_eq!(rows, vec!["the quick", "brown fox"]);
    }

    #[test]
    fn wrap_splits_a_word_that_cannot_fit() {
        let text = "abcdefghij";
        let rows: Vec<&str> = wrap_rows(text, 4).into_iter().map(|r| &text[r]).collect();
        assert_eq!(rows, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_gives_an_empty_line_its_own_row() {
        let text = "a\n\nb";
        let rows: Vec<&str> = wrap_rows(text, 10).into_iter().map(|r| &text[r]).collect();
        assert_eq!(rows, vec!["a", "", "b"]);
    }

    #[test]
    fn wrap_covers_an_empty_buffer() {
        assert_eq!(wrap_rows("", 10), vec![0..0]);
    }

    /// The caller walking the input history needs these two apart, so they are
    /// asserted apart here rather than left to whatever the caller assumes.
    #[test]
    fn moving_the_caret_and_changing_the_text_are_different_answers() {
        let mut c = Composer::new();
        typed(&mut c, "abc");
        assert_eq!(c.handle_key(&key(KeyCode::Left), 40), ComposerAction::Moved);
        assert_eq!(
            c.handle_key(&key(KeyCode::Backspace), 40),
            ComposerAction::Redraw
        );
        // A motion that had nowhere to go is still a motion, not an edit.
        c.handle_key(&key(KeyCode::Home), 40);
        assert_eq!(c.handle_key(&key(KeyCode::Left), 40), ComposerAction::Moved);
    }

    #[test]
    fn setting_the_text_replaces_it_and_leaves_the_caret_at_the_end() {
        let mut c = Composer::new();
        typed(&mut c, "draft");
        c.set_text("a recalled prompt");
        assert_eq!(c.text(), "a recalled prompt");
        assert_eq!(c.cursor_rowcol(40), (0, 17));
        typed(&mut c, "!");
        assert_eq!(c.text(), "a recalled prompt!");
    }

    #[test]
    fn a_wide_grapheme_counts_for_two_columns() {
        let mut c = Composer::new();
        c.insert_str("漢字");
        assert_eq!(c.cursor_rowcol(40), (0, 4));
    }

    #[test]
    fn slash_routing_only_keeps_commands_and_command_shaped_typos_local() {
        const COMMANDS: &[(&str, &str)] = &[("help", "list commands"), ("run", "run a command")];

        assert!(is_local_slash_input("/help", COMMANDS));
        assert!(is_local_slash_input("/run cargo test", COMMANDS));
        assert!(is_local_slash_input("/halp", COMMANDS));
        assert!(is_local_slash_input("/", COMMANDS));

        assert!(!is_local_slash_input(
            "/Users/name/work/openagents inspect this",
            COMMANDS
        ));
        assert!(!is_local_slash_input("/README.md", COMMANDS));
        assert!(!is_local_slash_input("/not a command", COMMANDS));
        assert!(!is_local_slash_input("ordinary prompt", COMMANDS));
    }
}
