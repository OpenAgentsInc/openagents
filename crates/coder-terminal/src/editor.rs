//! The composer's editing model: one draft, one caret, no renderer.
//!
//! `Editor` knows nothing about terminals. It holds the draft text and a
//! byte-offset caret, applies commands — insert, delete, move, wrap-aware
//! vertical motion — and reports where the caret landed. The composer draws
//! it; [`handle_key`][crate::handle_key] drives it; a shell owns it.
//!
//! Byte offsets are always grapheme boundaries: every motion and every edit
//! steps whole grapheme clusters, so a multi-byte emoji or a combined accent
//! is one character to the user.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::wrap::{byte_at_column, row_of, wrap_rows};

/// The fewest rows a composer occupies — an empty draft still reads as an
/// input field, not a hairline.
pub const ROWS_MIN: usize = 2;
/// The most rows a composer occupies before its window scrolls.
pub const ROWS_MAX: usize = 8;

/// The rows a composer shows: the draft's wrapped ranges, the index of the
/// first visible row, and how many rows the box draws.
pub struct Window {
    /// One byte range per wrapped row, in order.
    pub rows: Vec<Range<usize>>,
    /// The row `rows[scroll]` draws first; later rows follow until the box
    /// fills.
    pub scroll: usize,
    /// How many rows the box draws: the wrapped count clamped to
    /// `ROWS_MIN..=ROWS_MAX`. Rows past `rows.len()` draw blank, which is how
    /// an empty draft still reads as an input field.
    pub visible: usize,
}

/// A multi-line draft editor with readline motions and prompt history.
#[derive(Default)]
pub struct Editor {
    text: String,
    /// The caret, as a byte offset at a grapheme boundary.
    caret: usize,
    /// The column vertical motion remembers across a row change, so a trip
    /// through a short row does not lose the column the user meant.
    preferred_column: Option<usize>,
    /// The first wrapped row the window draws.
    scroll: usize,
    /// Submitted drafts, oldest first. The caret never enters here; `take`
    /// pushes and `previous_history`/`next_history` walk it.
    history: Vec<String>,
    /// Where a history walk stands: `None` on the live draft, `Some(i)` on
    /// `history[i]`.
    history_index: Option<usize>,
    /// The live draft stashed while a history walk browses; restored when the
    /// walk reaches the newest end again.
    stash: String,
}

impl Editor {
    /// An empty editor.
    pub fn new() -> Self {
        Self::default()
    }

    /// The draft text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret's byte offset.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Whether the draft is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Inserts a character at the caret.
    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.caret, ch);
        self.caret += ch.len_utf8();
        self.preferred_column = None;
    }

    /// Inserts a string at the caret — a paste, a completion, a fill-in.
    /// Newlines within `s` split the draft into lines as typed input would.
    pub fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.caret, s);
        self.caret += s.len();
        self.preferred_column = None;
    }

    /// Inserts a hard newline — one more line in the draft.
    pub fn newline(&mut self) {
        self.insert('\n');
    }

    /// Deletes the grapheme before the caret; a no-op at the start.
    pub fn backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        let prev = self
            .text
            .grapheme_indices(true)
            .take_while(|(offset, _)| *offset < self.caret)
            .last()
            .map(|(offset, _)| offset)
            .unwrap_or(0);
        self.text.replace_range(prev..self.caret, "");
        self.caret = prev;
        self.preferred_column = None;
    }

    /// Deletes the grapheme at the caret; a no-op at the end.
    pub fn delete(&mut self) {
        if let Some(len) = self
            .text
            .get(self.caret..)
            .and_then(|rest| rest.graphemes(true).next())
            .map(str::len)
        {
            self.text.replace_range(self.caret..self.caret + len, "");
        }
    }

    /// Deletes from the line start to the caret — readline's `Ctrl-U`.
    pub fn kill_line_start(&mut self) {
        let start = self.line_start();
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
        self.preferred_column = None;
    }

    /// Deletes from the caret to the line end — readline's `Ctrl-K`.
    pub fn kill_line_end(&mut self) {
        let end = self.line_end();
        if end > self.caret {
            self.text.replace_range(self.caret..end, "");
        }
    }

    /// Deletes the whitespace-delimited word before the caret — readline's
    /// `Ctrl-W`.
    pub fn kill_word(&mut self) {
        let word = self.word_back();
        self.text.replace_range(word..self.caret, "");
        self.caret = word;
        self.preferred_column = None;
    }

    /// Moves the caret one grapheme left.
    pub fn left(&mut self) -> bool {
        if self.caret == 0 {
            return false;
        }
        self.caret = self
            .text
            .grapheme_indices(true)
            .take_while(|(offset, _)| *offset < self.caret)
            .last()
            .map(|(offset, _)| offset)
            .unwrap_or(0);
        self.preferred_column = None;
        true
    }

    /// Moves the caret one grapheme right.
    pub fn right(&mut self) -> bool {
        let next = self
            .text
            .get(self.caret..)
            .and_then(|rest| rest.graphemes(true).next())
            .map(|g| self.caret + g.len());
        match next {
            Some(caret) => {
                self.caret = caret;
                self.preferred_column = None;
                true
            }
            None => false,
        }
    }

    /// Moves the caret to the start of the current line — `Home`, `Ctrl-A`.
    pub fn home(&mut self) {
        self.caret = self.line_start();
        self.preferred_column = None;
    }

    /// Moves the caret to the end of the current line — `End`, `Ctrl-E`.
    pub fn end(&mut self) {
        self.caret = self.line_end();
        self.preferred_column = None;
    }

    /// Moves the caret a word back — `Alt-B`. Words are runs of non-space
    /// characters, so the caret lands on the previous word's first grapheme.
    pub fn word_back_motion(&mut self) {
        self.caret = self.word_back();
        self.preferred_column = None;
    }

    /// Moves the caret a word forward — `Alt-F`.
    pub fn word_forward(&mut self) {
        let bytes = &self.text.as_bytes()[self.caret..];
        let mut index = 0;
        // Skip the word the caret is in or on, then any space after it.
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        self.caret += index;
        self.preferred_column = None;
    }

    /// Moves the caret a wrapped row up, keeping its column. Returns `false`
    /// on the first row, where a shell may instead walk history.
    pub fn up(&mut self, width: usize) -> bool {
        self.vertical(width, -1)
    }

    /// Moves the caret a wrapped row down, keeping its column. Returns
    /// `false` on the last row.
    pub fn down(&mut self, width: usize) -> bool {
        self.vertical(width, 1)
    }

    fn vertical(&mut self, width: usize, delta: isize) -> bool {
        let rows = wrap_rows(&self.text, width);
        let (row, column) = caret_row_column(&self.text, &rows, self.caret);
        let target = row as isize + delta;
        if target < 0 || target >= rows.len() as isize {
            return false;
        }
        let column = self.preferred_column.unwrap_or(column);
        self.preferred_column = Some(column);
        self.caret = byte_at_column(&self.text, rows[target as usize].clone(), column);
        true
    }

    /// Ends the draft: pushes it to history and returns it. An empty draft
    /// returns an empty string and enters no history.
    pub fn take(&mut self) -> String {
        let taken = std::mem::take(&mut self.text);
        if !taken.is_empty() {
            self.history.push(taken.clone());
        }
        self.caret = 0;
        self.scroll = 0;
        self.preferred_column = None;
        self.history_index = None;
        self.stash.clear();
        taken
    }

    /// Walks history toward older drafts — `Up` on the first row. Returns
    /// `false` when there is nothing older. The live draft is stashed and
    /// restored when the walk ends.
    pub fn previous_history(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let next = match self.history_index {
            None => {
                self.stash = std::mem::take(&mut self.text);
                self.history.len() - 1
            }
            Some(index) if index > 0 => index - 1,
            Some(_) => return false,
        };
        self.load_history(next);
        true
    }

    /// Walks history toward the live draft — `Down` on the last row.
    /// Reaching the end restores the stashed draft.
    pub fn next_history(&mut self) -> bool {
        match self.history_index {
            None => false,
            Some(index) if index + 1 < self.history.len() => {
                self.load_history(index + 1);
                true
            }
            Some(_) => {
                self.history_index = None;
                self.text = std::mem::take(&mut self.stash);
                self.caret = self.text.len();
                true
            }
        }
    }

    fn load_history(&mut self, index: usize) {
        self.history_index = Some(index);
        self.text = self.history[index].clone();
        self.caret = self.text.len();
        self.preferred_column = None;
    }

    /// The window a composer `width` cells wide draws: the draft's wrapped
    /// rows and the scroll offset that keeps the caret visible inside
    /// `ROWS_MIN..=ROWS_MAX` rows.
    pub fn window(&mut self, width: usize) -> Window {
        let rows = wrap_rows(&self.text, width);
        let visible = rows.len().clamp(ROWS_MIN, ROWS_MAX);
        let caret_row = row_of(&rows, self.caret);
        if caret_row < self.scroll {
            self.scroll = caret_row;
        } else if caret_row >= self.scroll + visible {
            self.scroll = caret_row + 1 - visible;
        }
        Window {
            rows,
            scroll: self.scroll,
            visible,
        }
    }

    /// The caret's wrapped row and display column at `width`, for the shell
    /// that places the terminal cursor.
    pub fn caret_row_column(&self, width: usize) -> (usize, usize) {
        let rows = wrap_rows(&self.text, width);
        caret_row_column(&self.text, &rows, self.caret)
    }

    /// The byte offset nearest the wrapped `row`, `column` — a mouse click
    /// lands the caret here.
    pub fn byte_at(&self, width: usize, row: usize, column: usize) -> usize {
        let rows = wrap_rows(&self.text, width);
        match rows.get(row) {
            Some(range) => byte_at_column(&self.text, range.clone(), column),
            None => self.text.len(),
        }
    }

    /// Places the caret — a mouse click resolved to a byte offset.
    pub fn set_caret(&mut self, caret: usize) {
        self.caret = caret.min(self.text.len());
        while !self.text.is_char_boundary(self.caret) {
            self.caret -= 1;
        }
        self.preferred_column = None;
    }

    fn line_start(&self) -> usize {
        self.text[..self.caret].rfind('\n').map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.caret..]
            .find('\n')
            .map_or(self.text.len(), |i| self.caret + i)
    }

    fn word_back(&self) -> usize {
        let bytes = &self.text.as_bytes()[..self.caret];
        let mut index = self.caret;
        while index > 0 && bytes[index - 1].is_ascii_whitespace() {
            index -= 1;
        }
        while index > 0 && !bytes[index - 1].is_ascii_whitespace() {
            index -= 1;
        }
        index
    }
}

fn caret_row_column(text: &str, rows: &[Range<usize>], caret: usize) -> (usize, usize) {
    let row = row_of(rows, caret);
    let column = text[rows[row].start..caret]
        .graphemes(true)
        .map(|g| g.width().max(1))
        .sum();
    (row, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_reads_back() {
        let mut editor = Editor::new();
        editor.insert_str("hello");
        assert_eq!(editor.text(), "hello");
        assert_eq!(editor.caret(), 5);
    }

    #[test]
    fn backspace_steps_whole_graphemes() {
        let mut editor = Editor::new();
        editor.insert_str("a🦀b");
        editor.backspace();
        assert_eq!(editor.text(), "a🦀");
        editor.backspace();
        assert_eq!(editor.text(), "a");
        assert_eq!(editor.caret(), 1);
    }

    #[test]
    fn kill_line_start_respects_newlines() {
        let mut editor = Editor::new();
        editor.insert_str("one\ntwo");
        editor.kill_line_start();
        assert_eq!(editor.text(), "one\n");
    }

    #[test]
    fn kill_word_eats_space_then_word() {
        let mut editor = Editor::new();
        editor.insert_str("alpha beta  ");
        editor.kill_word();
        assert_eq!(editor.text(), "alpha ");
    }

    #[test]
    fn word_motions_land_on_boundaries() {
        let mut editor = Editor::new();
        editor.insert_str("one two  three");
        editor.word_back_motion();
        assert_eq!(editor.caret(), 9);
        editor.word_back_motion();
        assert_eq!(editor.caret(), 4);
        editor.word_forward();
        assert_eq!(editor.caret(), 9);
    }

    #[test]
    fn home_and_end_stay_on_the_logical_line() {
        let mut editor = Editor::new();
        editor.insert_str("one\ntwo\nthree");
        editor.set_caret(5);
        editor.home();
        assert_eq!(editor.caret(), 4);
        editor.end();
        assert_eq!(editor.caret(), 7);
    }

    #[test]
    fn vertical_motion_keeps_a_column_across_short_rows() {
        let mut editor = Editor::new();
        editor.insert_str("abcdefgh\nx\nabcdefgh");
        // Width 4 wraps the long lines: rows are "abcd" "efgh" "x" "abcd" "efgh".
        editor.set_caret(3); // row 0, column 3
        assert!(editor.down(4)); // row 1, byte 7
        assert!(editor.down(4)); // row 2, the short row — column clamps to its end
        assert_eq!(editor.caret(), 10);
        assert!(editor.down(4)); // row 3 — the remembered column returns
        assert_eq!(editor.caret(), 14);
    }

    #[test]
    fn up_and_down_report_the_edges() {
        let mut editor = Editor::new();
        editor.insert_str("only");
        assert!(!editor.up(10));
        assert!(!editor.down(10));
    }

    #[test]
    fn take_returns_and_records() {
        let mut editor = Editor::new();
        editor.insert_str("ls");
        assert_eq!(editor.take(), "ls");
        assert!(editor.is_empty());
        assert!(editor.previous_history());
        assert_eq!(editor.text(), "ls");
    }

    #[test]
    fn history_walks_stash_and_restore_the_draft() {
        let mut editor = Editor::new();
        editor.insert_str("first");
        editor.take();
        editor.insert_str("second");
        editor.take();
        editor.insert_str("draft");
        assert!(editor.previous_history());
        assert_eq!(editor.text(), "second");
        assert!(editor.previous_history());
        assert_eq!(editor.text(), "first");
        assert!(!editor.previous_history()); // oldest reached
        assert!(editor.next_history());
        assert_eq!(editor.text(), "second");
        assert!(editor.next_history());
        assert_eq!(editor.text(), "draft"); // the stash returns
        assert!(!editor.next_history());
    }

    #[test]
    fn the_window_scrolls_to_keep_the_caret_visible() {
        let mut editor = Editor::new();
        editor.insert_str("aa bb cc dd ee ff gg hh ii jj kk ll");
        let window = editor.window(4);
        // Rows of 4 cells: 8 rows, over ROWS_MAX, so the window shows the
        // tail where the caret sits.
        assert!(window.scroll > 0);
        let caret_row = row_of(&window.rows, editor.caret());
        assert!(caret_row >= window.scroll && caret_row < window.scroll + ROWS_MAX);
    }

    #[test]
    fn caret_row_column_counts_wrapped_cells() {
        let mut editor = Editor::new();
        editor.insert_str("the quick brown");
        editor.set_caret(10); // start of "brown" after wrap at width 10
        let (row, column) = editor.caret_row_column(10);
        assert_eq!((row, column), (1, 0));
    }

    #[test]
    fn a_combining_accent_edits_as_one_grapheme() {
        // "e" + U+0301 combining acute, then "x".
        let mut editor = Editor::new();
        editor.insert_str("e\u{301}x");
        assert!(editor.left());
        assert_eq!(editor.caret(), 3);
        editor.backspace();
        assert_eq!(editor.text(), "x");
        assert_eq!(editor.caret(), 0);
    }

    #[test]
    fn a_zwj_emoji_sequence_is_one_step_and_one_delete() {
        // Family emoji: four code points joined by U+200D.
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let mut editor = Editor::new();
        editor.insert_str(family);
        editor.insert_str("!");
        editor.home();
        assert!(editor.right());
        assert_eq!(editor.caret(), family.len());
        editor.home();
        editor.delete();
        assert_eq!(editor.text(), "!");
    }

    #[test]
    fn wide_characters_count_two_cells_toward_the_caret_column() {
        let mut editor = Editor::new();
        editor.insert_str("日本語");
        editor.left();
        let (row, column) = editor.caret_row_column(40);
        assert_eq!((row, column), (0, 4));
        editor.backspace();
        assert_eq!(editor.text(), "日語");
    }

    #[test]
    fn a_resize_reflows_the_draft_and_keeps_the_caret_in_view() {
        let mut editor = Editor::new();
        editor.insert_str("one two three four five six seven eight nine ten");
        let wide = editor.window(80);
        assert_eq!(wide.rows.len(), 1);
        assert_eq!(wide.scroll, 0);
        let narrow = editor.window(6);
        assert!(narrow.rows.len() > ROWS_MAX);
        let caret_row = row_of(&narrow.rows, editor.caret());
        assert!(caret_row >= narrow.scroll && caret_row < narrow.scroll + narrow.visible);
        // Widening again drops the scroll back to the top row.
        editor.home();
        editor.set_caret(0);
        let wide = editor.window(80);
        assert_eq!(wide.rows.len(), 1);
        assert_eq!(wide.scroll, 0);
    }
}
