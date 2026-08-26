//! Grapheme-correct text editing over a flat buffer.
//!
//! # Provenance
//!
//! Ported from `crates/codegen/xai-ratatui-textarea/src/editor.rs` in
//! grok-build, © 2023-2026 SpaceXAI, Apache-2.0. Reference clone read at
//! commit `07b2f7144fd5c5c9d3dd1966937a87852d2dbdb8`. The Apache-2.0 text and
//! the copyright line are reproduced in `LICENSE-APACHE-xai` beside this file.
//!
//! Trimmed against the original, and only by removing things this crate has no
//! use for:
//!
//! - The `atomic_byte_ranges` parameter threaded through every function. It
//!   exists upstream so a rich element — an image chip — deletes as one unit.
//!   Every call site here passed an empty slice, under which
//!   `previous_atomic_boundary` is exactly `previous_grapheme_boundary`,
//!   `atomic_word_class` is exactly `word_class`, and the rest are identities,
//!   so the parameter is gone and the callees are collapsed into their
//!   grapheme forms.
//! - `single_line_viewport`, which scrolls a one-line field horizontally. The
//!   composer soft-wraps instead, and this was the file's only use of
//!   `unicode-width`.
//! - `EditPlan`'s buffer identity and generation counter, along with
//!   `apply_plan`, `validate_plan` and `ApplyEditPlanError`. Those guard a
//!   plan computed against one buffer from being applied to another; nothing
//!   here holds a plan across a mutation.
//! - The `Movement` vocabulary and `resolve_movement`, which need wrap
//!   geometry the upstream widget owns. `super::Composer` does its own
//!   vertical motion over its own wrap rows.
//!
//! What is kept is kept as written: the plan-then-apply split, the word-class
//! run scanning, the line-edge chaining that Ctrl+A and Ctrl+E rely on, and
//! all of the grapheme boundary math. The upstream file's one let-chain lived
//! in the atomic-range normalizer that this port drops, so nothing here needs
//! rewriting for this workspace's edition 2021.

use std::ops::{Deref, Range};

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordStyle {
    /// Words break at punctuation as well as whitespace.
    Small,
    /// Words break only at whitespace, the way readline's Ctrl+W does.
    WhitespaceDelimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCommand {
    Insert(char),
    MoveGraphemeLeft,
    MoveGraphemeRight,
    MoveWordLeft(WordStyle),
    MoveWordRight(WordStyle),
    MoveLogicalLineStart,
    MoveLogicalLineEnd,
    DeleteGraphemeBackward,
    DeleteGraphemeForward,
    DeleteWordBackward(WordStyle),
    DeleteWordForward(WordStyle),
    DeleteToLineStart,
    DeleteToLineEnd,
}

impl EditCommand {
    /// True for the commands that only move the caret.
    pub fn is_navigation(self) -> bool {
        matches!(
            self,
            Self::MoveGraphemeLeft
                | Self::MoveGraphemeRight
                | Self::MoveWordLeft(_)
                | Self::MoveWordRight(_)
                | Self::MoveLogicalLineStart
                | Self::MoveLogicalLineEnd
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditDelta {
    pub replaced_byte_range: Range<usize>,
    pub inserted_byte_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    Unchanged,
    CursorOnly,
    TextOnly(EditDelta),
    TextAndCursor(EditDelta),
}

impl EditOutcome {
    fn from_changes(delta: Option<EditDelta>, cursor_changed: bool) -> Self {
        match (delta, cursor_changed) {
            (None, false) => Self::Unchanged,
            (None, true) => Self::CursorOnly,
            (Some(delta), false) => Self::TextOnly(delta),
            (Some(delta), true) => Self::TextAndCursor(delta),
        }
    }

    /// Whether the frame needs redrawing.
    pub fn changed(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    /// Whether the text itself moved, as opposed to only the caret.
    pub fn text_changed(&self) -> bool {
        matches!(self, Self::TextOnly(_) | Self::TextAndCursor(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostEditCursorAffinity {
    /// The caret lands exactly where the plan says.
    Exact,
    /// The caret is pushed right to the next grapheme boundary if the edit
    /// left it inside a cluster.
    Right,
}

/// What an edit would do, computed without doing it.
#[derive(Debug, Clone)]
pub struct EditPlan {
    replaced_byte_range: Range<usize>,
    replacement: String,
    removed_text: String,
    cursor_byte: usize,
    cursor_affinity: PostEditCursorAffinity,
}

impl EditPlan {
    pub fn removed_text(&self) -> &str {
        &self.removed_text
    }
}

/// A string and a caret byte offset, where the offset is always on a grapheme
/// boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditBuffer {
    text: String,
    cursor_byte: usize,
}

impl Deref for EditBuffer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.text()
    }
}

impl EditBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor_byte = text.len();
        Self { text, cursor_byte }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn cursor_byte(&self) -> usize {
        self.cursor_byte
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Empty the buffer and hand back what was in it.
    pub fn take(&mut self) -> String {
        self.cursor_byte = 0;
        std::mem::take(&mut self.text)
    }

    /// External cursor requests use nearest grapheme boundaries; ties go left
    /// for determinism.
    pub fn set_cursor_byte(&mut self, cursor_byte: usize) -> EditOutcome {
        let old_cursor = self.cursor_byte;
        self.cursor_byte = normalize_external_cursor(&self.text, cursor_byte);
        EditOutcome::from_changes(None, self.cursor_byte != old_cursor)
    }

    pub fn insert_str(&mut self, text: &str) -> EditOutcome {
        let plan = self.plan_replace_byte_range(self.cursor_byte..self.cursor_byte, text);
        self.apply_plan(&plan)
    }

    pub fn plan_replace_byte_range(&self, range: Range<usize>, replacement: &str) -> EditPlan {
        let range = normalize_replacement_range(&self.text, range);
        let cursor_byte = self.cursor_byte;
        let next_cursor = if cursor_byte < range.start {
            cursor_byte
        } else if cursor_byte <= range.end {
            range.start + replacement.len()
        } else {
            cursor_byte - (range.end - range.start) + replacement.len()
        };
        self.make_plan(
            range,
            replacement.to_owned(),
            next_cursor,
            PostEditCursorAffinity::Right,
        )
    }

    pub fn plan_command(&self, command: EditCommand) -> EditPlan {
        let cursor_byte = self.cursor_byte;
        match command {
            EditCommand::Insert(character) => self.make_plan(
                cursor_byte..cursor_byte,
                character.to_string(),
                cursor_byte + character.len_utf8(),
                PostEditCursorAffinity::Right,
            ),
            EditCommand::MoveGraphemeLeft => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                previous_grapheme_boundary(&self.text, cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::MoveGraphemeRight => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                next_grapheme_boundary(&self.text, cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::MoveWordLeft(style) => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                self.previous_word_boundary(style, cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::MoveWordRight(style) => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                self.next_word_boundary(style, cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::MoveLogicalLineStart => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                self.logical_line_start_target(cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::MoveLogicalLineEnd => self.make_plan(
                cursor_byte..cursor_byte,
                String::new(),
                self.logical_line_end_target(cursor_byte),
                PostEditCursorAffinity::Exact,
            ),
            EditCommand::DeleteGraphemeBackward => {
                let start = previous_grapheme_boundary(&self.text, cursor_byte);
                self.make_plan(
                    start..cursor_byte,
                    String::new(),
                    start,
                    PostEditCursorAffinity::Right,
                )
            }
            EditCommand::DeleteGraphemeForward => {
                let end = next_grapheme_boundary(&self.text, cursor_byte);
                self.make_plan(
                    cursor_byte..end,
                    String::new(),
                    cursor_byte,
                    PostEditCursorAffinity::Right,
                )
            }
            EditCommand::DeleteWordBackward(style) => {
                let start = self.previous_word_boundary(style, cursor_byte);
                self.make_plan(
                    start..cursor_byte,
                    String::new(),
                    start,
                    PostEditCursorAffinity::Right,
                )
            }
            EditCommand::DeleteWordForward(style) => {
                let end = self.next_word_boundary(style, cursor_byte);
                self.make_plan(
                    cursor_byte..end,
                    String::new(),
                    cursor_byte,
                    PostEditCursorAffinity::Right,
                )
            }
            EditCommand::DeleteToLineStart => {
                let line_start = self.line_start_at(cursor_byte);
                let start = if cursor_byte == line_start {
                    previous_grapheme_boundary(&self.text, line_start)
                } else {
                    line_start
                };
                self.make_plan(
                    start..cursor_byte,
                    String::new(),
                    start,
                    PostEditCursorAffinity::Right,
                )
            }
            EditCommand::DeleteToLineEnd => {
                let line_end = self.line_end_from(cursor_byte);
                let start = cursor_byte.min(line_end);
                let end = if cursor_byte >= line_end {
                    self.line_ending_at(line_end)
                        .map_or(line_end, |range| range.end)
                } else {
                    line_end
                };
                self.make_plan(
                    start..end,
                    String::new(),
                    start,
                    PostEditCursorAffinity::Right,
                )
            }
        }
    }

    pub fn apply(&mut self, command: EditCommand) -> EditOutcome {
        let plan = self.plan_command(command);
        self.apply_plan(&plan)
    }

    fn make_plan(
        &self,
        replaced_byte_range: Range<usize>,
        replacement: String,
        cursor_byte: usize,
        cursor_affinity: PostEditCursorAffinity,
    ) -> EditPlan {
        let removed_text = self.text[replaced_byte_range.clone()].to_owned();
        EditPlan {
            replaced_byte_range,
            replacement,
            removed_text,
            cursor_byte,
            cursor_affinity,
        }
    }

    pub fn apply_plan(&mut self, plan: &EditPlan) -> EditOutcome {
        let old_cursor = self.cursor_byte;
        let text_changed = plan.removed_text != plan.replacement;
        let inserted_len = plan.replacement.len();
        if text_changed {
            self.text
                .replace_range(plan.replaced_byte_range.clone(), &plan.replacement);
        }
        self.cursor_byte = match plan.cursor_affinity {
            PostEditCursorAffinity::Exact => plan.cursor_byte,
            PostEditCursorAffinity::Right => ceil_grapheme_boundary(&self.text, plan.cursor_byte),
        };
        let cursor_changed = self.cursor_byte != old_cursor;
        let delta = if text_changed {
            Some(EditDelta {
                inserted_byte_range: plan.replaced_byte_range.start
                    ..(plan.replaced_byte_range.start + inserted_len),
                replaced_byte_range: plan.replaced_byte_range.clone(),
            })
        } else {
            None
        };
        EditOutcome::from_changes(delta, cursor_changed)
    }

    fn previous_word_boundary(&self, style: WordStyle, cursor_byte: usize) -> usize {
        let mut position = cursor_byte;
        while position > 0 {
            let previous = previous_grapheme_boundary(&self.text, position);
            if word_class(&self.text[previous..position], style) == Some(WordClass::Whitespace) {
                position = previous;
            } else {
                break;
            }
        }

        if position == 0 {
            return 0;
        }

        let previous = previous_grapheme_boundary(&self.text, position);
        let target_class = word_class(&self.text[previous..position], style);
        while position > 0 {
            let previous = previous_grapheme_boundary(&self.text, position);
            if word_class(&self.text[previous..position], style) != target_class {
                break;
            }
            position = previous;
        }
        position
    }

    fn next_word_boundary(&self, style: WordStyle, cursor_byte: usize) -> usize {
        let mut position = cursor_byte;
        while position < self.text.len() {
            let next = next_grapheme_boundary(&self.text, position);
            if word_class(&self.text[position..next], style) == Some(WordClass::Whitespace) {
                position = next;
            } else {
                break;
            }
        }

        if position == self.text.len() {
            return position;
        }

        let next = next_grapheme_boundary(&self.text, position);
        let target_class = word_class(&self.text[position..next], style);
        while position < self.text.len() {
            let next = next_grapheme_boundary(&self.text, position);
            if word_class(&self.text[position..next], style) != target_class {
                break;
            }
            position = next;
        }
        position
    }

    /// Ctrl+A at the start of a line chains to the start of the line above.
    fn logical_line_start_target(&self, cursor_byte: usize) -> usize {
        let line_start = self.line_start_at(cursor_byte);
        if cursor_byte == line_start && line_start > 0 {
            let previous_line_end = previous_grapheme_boundary(&self.text, line_start);
            self.line_start_at(previous_line_end)
        } else {
            line_start
        }
    }

    /// Ctrl+E at the end of a line chains to the end of the line below.
    fn logical_line_end_target(&self, cursor_byte: usize) -> usize {
        let line_end = self.line_end_from(cursor_byte);
        if cursor_byte == line_end {
            self.line_ending_at(line_end)
                .map_or(line_end, |range| self.line_end_from(range.end))
        } else {
            line_end
        }
    }

    pub fn line_start_at(&self, cursor_byte: usize) -> usize {
        let cursor_byte = cursor_byte.min(self.text.len());
        (0..cursor_byte)
            .rev()
            .find(|position| self.text.as_bytes()[*position] == b'\n')
            .map_or(0, |position| position + 1)
    }

    pub fn line_end_from(&self, cursor_byte: usize) -> usize {
        let cursor_byte = cursor_byte.min(self.text.len());
        (cursor_byte..self.text.len())
            .find(|position| self.text.as_bytes()[*position] == b'\n')
            .map_or(self.text.len(), |line_feed| {
                if line_feed > 0 && self.text.as_bytes()[line_feed - 1] == b'\r' {
                    line_feed - 1
                } else {
                    line_feed
                }
            })
    }

    fn line_ending_at(&self, line_end: usize) -> Option<Range<usize>> {
        let remaining = self.text.get(line_end..)?;
        if remaining.starts_with("\r\n") {
            Some(line_end..line_end + 2)
        } else if remaining.starts_with('\n') {
            Some(line_end..line_end + 1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Whitespace,
    Word,
    Punctuation,
}

fn word_class(grapheme: &str, style: WordStyle) -> Option<WordClass> {
    let character = grapheme.chars().next()?;
    if character.is_whitespace() {
        Some(WordClass::Whitespace)
    } else if style == WordStyle::WhitespaceDelimited
        || character.is_alphanumeric()
        || character == '_'
    {
        Some(WordClass::Word)
    } else {
        Some(WordClass::Punctuation)
    }
}

fn normalize_replacement_range(text: &str, range: Range<usize>) -> Range<usize> {
    let raw_start = range.start.min(range.end).min(text.len());
    let raw_end = range.start.max(range.end).min(text.len());
    if raw_start == raw_end {
        let cursor = normalize_external_cursor(text, raw_start);
        return cursor..cursor;
    }
    floor_grapheme_boundary(text, raw_start)..ceil_grapheme_boundary(text, raw_end)
}

pub fn floor_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if byte == text.len() {
        return byte;
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= byte)
        .last()
        .unwrap_or(0)
}

pub fn ceil_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if byte == text.len() {
        return byte;
    }
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|index| *index >= byte)
        .unwrap_or(text.len())
}

fn normalize_external_cursor(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let before = floor_grapheme_boundary(text, byte);
    let after = ceil_grapheme_boundary(text, byte);
    if byte - before <= after - byte {
        before
    } else {
        after
    }
}

pub fn previous_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if byte == 0 {
        return 0;
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    match cursor.prev_boundary(text, 0) {
        Ok(Some(boundary)) => boundary,
        Ok(None) => 0,
        Err(_) => floor_grapheme_boundary(text, byte.saturating_sub(1)),
    }
}

pub fn next_grapheme_boundary(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if byte == text.len() {
        return byte;
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    match cursor.next_boundary(text, 0) {
        Ok(Some(boundary)) => boundary,
        Ok(None) => text.len(),
        Err(_) => ceil_grapheme_boundary(text, byte.saturating_add(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str, cursor: usize) -> EditBuffer {
        let mut b = EditBuffer::from_text(text);
        let _ = b.set_cursor_byte(cursor);
        b
    }

    #[test]
    fn insert_lands_at_the_caret() {
        let mut b = buffer("ac", 1);
        let _ = b.apply(EditCommand::Insert('b'));
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursor_byte(), 2);
    }

    #[test]
    fn backspace_removes_a_whole_grapheme_cluster() {
        // A ZWJ emoji sequence is many bytes and one grapheme. Deleting by
        // char or by byte would leave a broken cluster on screen.
        let mut b = EditBuffer::from_text("hi 👩‍💻");
        let before = b.text().len();
        let _ = b.apply(EditCommand::DeleteGraphemeBackward);
        assert_eq!(b.text(), "hi ");
        assert!(before - b.text().len() > 4);
    }

    #[test]
    fn ctrl_w_rubs_out_to_whitespace_not_to_punctuation() {
        let mut b = EditBuffer::from_text("git commit -m hello-world");
        let _ = b.apply(EditCommand::DeleteWordBackward(
            WordStyle::WhitespaceDelimited,
        ));
        assert_eq!(b.text(), "git commit -m ");
    }

    #[test]
    fn alt_backspace_stops_at_punctuation() {
        let mut b = EditBuffer::from_text("hello-world");
        let _ = b.apply(EditCommand::DeleteWordBackward(WordStyle::Small));
        assert_eq!(b.text(), "hello-");
    }

    #[test]
    fn ctrl_u_kills_to_the_start_of_the_line() {
        let mut b = buffer("hello world", 5);
        let _ = b.apply(EditCommand::DeleteToLineStart);
        assert_eq!(b.text(), " world");
        assert_eq!(b.cursor_byte(), 0);
    }

    #[test]
    fn line_edges_respect_embedded_newlines() {
        let b = buffer("one\ntwo", 5);
        assert_eq!(b.line_start_at(5), 4);
        assert_eq!(b.line_end_from(5), 7);
    }

    #[test]
    fn grapheme_moves_step_over_a_cluster_in_one_go() {
        let mut b = buffer("👩‍💻x", 0);
        let _ = b.apply(EditCommand::MoveGraphemeRight);
        assert!(b.cursor_byte() > 4);
        let _ = b.apply(EditCommand::MoveGraphemeLeft);
        assert_eq!(b.cursor_byte(), 0);
    }
}
