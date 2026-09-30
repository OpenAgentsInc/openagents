//! Bounded text editing for native adapters, without platform objects.
//!
//! This reimplements the editing design of Zeron's `ComposerInput`: directional
//! selection, grapheme and word movement, coalesced undo, and one undo step for
//! an IME composition. Offsets are UTF-8 bytes. Native UTF-16 boundaries must
//! pass the checked conversion helpers before an adapter applies an edit.
//! Application submission and view identity remain outside the editor.

use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

pub mod mirror;

const HISTORY_STEPS: usize = 128;
const HISTORY_BYTES: usize = 512 * 1024;
const COALESCE_MS: u64 = 500;

/// Directional selection. `anchor` stays fixed while `caret` moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub caret: usize,
}

impl Selection {
    pub fn collapsed(offset: usize) -> Self {
        Self {
            anchor: offset,
            caret: offset,
        }
    }

    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
}

/// Movement in logical text; visual line movement belongs to the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    PreviousGrapheme,
    NextGrapheme,
    PreviousWord,
    NextWord,
    LineStart,
    LineEnd,
    Start,
    End,
}

/// Whether a replacement may join the preceding undo step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Typing,
    Backspace,
    Delete,
    /// Paste, cut, or a programmatic replacement: always its own undo step.
    Replace,
}

/// Refusals contain no draft text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    Bound,
    TooLong,
    Boundary,
    Composing,
    RevisionExhausted,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bound => "text editing requires a byte bound from 1 to 64 KiB",
            Self::TooLong => "text exceeds the editor's byte bound",
            Self::Boundary => "text offset does not name a valid character boundary",
            Self::Composing => "finish the input composition before this edit",
            Self::RevisionExhausted => "text editing revision is exhausted",
        })
    }
}

impl std::error::Error for EditError {}

#[derive(Clone)]
struct Snapshot {
    text: String,
    selection: Selection,
}

#[derive(Default)]
struct History {
    entries: VecDeque<Snapshot>,
    bytes: usize,
}

impl History {
    fn push(&mut self, snapshot: Snapshot) {
        self.bytes += snapshot.text.len();
        self.entries.push_back(snapshot);
        while self.entries.len() > HISTORY_STEPS || self.bytes > HISTORY_BYTES {
            if let Some(removed) = self.entries.pop_front() {
                self.bytes -= removed.text.len();
            }
        }
    }

    fn pop(&mut self) -> Option<Snapshot> {
        let snapshot = self.entries.pop_back()?;
        self.bytes -= snapshot.text.len();
        Some(snapshot)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

struct Composition {
    range: Range<usize>,
    before: Snapshot,
}

#[derive(Clone, Copy)]
struct Run {
    kind: EditKind,
    tail: usize,
    at_ms: u64,
}

/// An adapter's local draft. An application rerender must not recreate it.
pub struct Editor {
    text: String,
    selection: Selection,
    max_bytes: usize,
    revision: u64,
    composition: Option<Composition>,
    undo: History,
    redo: History,
    run: Option<Run>,
}

impl fmt::Debug for Editor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Editor")
            .field("bytes", &self.text.len())
            .field("selection", &self.selection)
            .field("revision", &self.revision)
            .field("composing", &self.composition.is_some())
            .finish_non_exhaustive()
    }
}

impl Editor {
    pub fn new(text: &str, max_bytes: usize) -> Result<Self, EditError> {
        if max_bytes == 0 || max_bytes > crate::input::MAX_INPUT_VALUE_BYTES {
            return Err(EditError::Bound);
        }
        if text.len() > max_bytes {
            return Err(EditError::TooLong);
        }
        Ok(Self {
            text: text.to_owned(),
            selection: Selection::collapsed(text.len()),
            max_bytes,
            revision: 0,
            composition: None,
            undo: History::default(),
            redo: History::default(),
            run: None,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }
    pub fn is_composing(&self) -> bool {
        self.composition.is_some()
    }
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.composition.as_ref().map(|c| c.range.clone())
    }
    /// Whether undo would change the draft: a history step or a composition.
    pub fn can_undo(&self) -> bool {
        self.composition.is_some() || !self.undo.entries.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        self.composition.is_none() && !self.redo.entries.is_empty()
    }
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.range()]
    }

    /// Native selection accepts character boundaries, including inside a
    /// combining sequence. Keyboard movement and deletion use graphemes.
    pub fn select(&mut self, selection: Selection) -> Result<(), EditError> {
        self.not_composing()?;
        check_range(&self.text, &selection.range())?;
        self.advance()?;
        self.selection = selection;
        self.run = None;
        Ok(())
    }

    pub fn select_all(&mut self) -> Result<(), EditError> {
        self.select(Selection {
            anchor: 0,
            caret: self.text.len(),
        })
    }

    pub fn move_caret(&mut self, movement: Movement, extend: bool) -> Result<(), EditError> {
        self.not_composing()?;
        let range = self.selection.range();
        let caret = match (movement, extend, range.is_empty()) {
            (Movement::PreviousGrapheme, false, false) => range.start,
            (Movement::NextGrapheme, false, false) => range.end,
            _ => self.destination(movement),
        };
        let caret = self.grapheme_boundary(
            caret,
            matches!(
                movement,
                Movement::NextGrapheme | Movement::NextWord | Movement::LineEnd | Movement::End
            ),
        );
        self.select(Selection {
            anchor: if extend { self.selection.anchor } else { caret },
            caret,
        })
    }

    /// Replace a native range, or the current selection. `at_ms` is a monotonic
    /// adapter clock; a backwards clock breaks undo coalescing.
    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        kind: EditKind,
        at_ms: u64,
    ) -> Result<(), EditError> {
        self.not_composing()?;
        let range = range.unwrap_or_else(|| self.selection.range());
        self.check_replacement(&range, text)?;
        if range.is_empty() && text.is_empty() {
            return Ok(());
        }
        self.advance()?;
        let single = match kind {
            EditKind::Typing => range.is_empty() && text.graphemes(true).count() == 1,
            EditKind::Backspace | EditKind::Delete => {
                text.is_empty() && self.text[range.clone()].graphemes(true).count() == 1
            }
            EditKind::Replace => false,
        };
        let merge = single
            && self.run.is_some_and(|run| {
                run.kind == kind
                    && at_ms
                        .checked_sub(run.at_ms)
                        .is_some_and(|dt| dt < COALESCE_MS)
                    && match kind {
                        EditKind::Typing => {
                            range.start == run.tail && !text.chars().any(char::is_whitespace)
                        }
                        EditKind::Backspace => range.end == run.tail,
                        EditKind::Delete => range.start == run.tail,
                        EditKind::Replace => false,
                    }
            });
        if !merge {
            self.undo.push(self.snapshot());
        }
        self.redo.clear();
        self.apply(range.clone(), text);
        self.run = single.then_some(Run {
            kind,
            tail: if kind == EditKind::Typing {
                range.start + text.len()
            } else {
                range.start
            },
            at_ms,
        });
        Ok(())
    }

    /// Delete the selection or one whole grapheme beside the caret.
    pub fn delete(&mut self, backwards: bool, at_ms: u64) -> Result<(), EditError> {
        self.not_composing()?;
        let mut range = self.selection.range();
        if range.is_empty() {
            let caret = range.start;
            if backwards {
                range = self.destination(Movement::PreviousGrapheme)
                    ..self.grapheme_boundary(caret, true);
            } else {
                range =
                    self.grapheme_boundary(caret, false)..self.destination(Movement::NextGrapheme);
            }
        }
        self.replace(
            Some(range),
            "",
            if backwards {
                EditKind::Backspace
            } else {
                EditKind::Delete
            },
            at_ms,
        )
    }

    /// Update an IME's provisional text. The selection is relative to `text`,
    /// in UTF-8 bytes, and may sit on any checked character boundary.
    pub fn preedit(&mut self, text: &str, selection: Selection) -> Result<(), EditError> {
        check_range(text, &selection.range())?;
        let range = self
            .marked_range()
            .unwrap_or_else(|| self.selection.range());
        self.check_replacement(&range, text)?;
        self.advance()?;
        let before = self
            .composition
            .take()
            .map(|c| c.before)
            .unwrap_or_else(|| self.snapshot());
        self.apply(range.clone(), text);
        self.selection = Selection {
            anchor: range.start + selection.anchor,
            caret: range.start + selection.caret,
        };
        self.composition = Some(Composition {
            range: range.start..range.start + text.len(),
            before,
        });
        self.run = None;
        Ok(())
    }

    /// Commit marked text as one undo step. Without preedit, this is a single
    /// replacement of the current selection, as for an IME's direct commit.
    pub fn commit(&mut self, text: &str) -> Result<(), EditError> {
        let range = self
            .marked_range()
            .unwrap_or_else(|| self.selection.range());
        self.check_replacement(&range, text)?;
        self.advance()?;
        let before = self
            .composition
            .take()
            .map(|c| c.before)
            .unwrap_or_else(|| self.snapshot());
        self.undo.push(before);
        self.redo.clear();
        self.apply(range, text);
        self.run = None;
        Ok(())
    }

    /// Cancel provisional input and restore both the text and selection.
    pub fn cancel_composition(&mut self) -> Result<bool, EditError> {
        if self.composition.is_none() {
            return Ok(false);
        }
        self.advance()?;
        let before = self
            .composition
            .take()
            .expect("composition was checked")
            .before;
        self.restore(before);
        Ok(true)
    }

    pub fn undo(&mut self) -> Result<bool, EditError> {
        if self.is_composing() {
            return self.cancel_composition();
        }
        if self.undo.entries.is_empty() {
            return Ok(false);
        }
        self.advance()?;
        let before = self.undo.pop().expect("undo history was checked");
        self.redo.push(self.snapshot());
        self.restore(before);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, EditError> {
        self.not_composing()?;
        if self.redo.entries.is_empty() {
            return Ok(false);
        }
        self.advance()?;
        let next = self.redo.pop().expect("redo history was checked");
        self.undo.push(self.snapshot());
        self.restore(next);
        Ok(true)
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            selection: self.selection,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.selection = snapshot.selection;
        self.run = None;
    }

    fn advance(&mut self) -> Result<(), EditError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(EditError::RevisionExhausted)?;
        Ok(())
    }

    fn not_composing(&self) -> Result<(), EditError> {
        if self.is_composing() {
            Err(EditError::Composing)
        } else {
            Ok(())
        }
    }

    fn check_replacement(&self, range: &Range<usize>, text: &str) -> Result<(), EditError> {
        check_range(&self.text, range)?;
        if text.len() > self.max_bytes - (self.text.len() - range.len()) {
            return Err(EditError::TooLong);
        }
        Ok(())
    }

    fn apply(&mut self, range: Range<usize>, text: &str) {
        let caret = range.start + text.len();
        self.text.replace_range(range, text);
        self.selection = Selection::collapsed(caret);
    }

    fn grapheme_boundary(&self, offset: usize, forward: bool) -> usize {
        if offset == self.text.len() {
            return offset;
        }
        let mut previous = 0;
        for (boundary, _) in self.text.grapheme_indices(true) {
            if boundary == offset {
                return offset;
            }
            if boundary > offset {
                return if forward { boundary } else { previous };
            }
            previous = boundary;
        }
        if forward { self.text.len() } else { previous }
    }

    fn destination(&self, movement: Movement) -> usize {
        let caret = self.selection.caret;
        match movement {
            Movement::PreviousGrapheme => self
                .text
                .grapheme_indices(true)
                .rev()
                .find_map(|(i, _)| (i < caret).then_some(i))
                .unwrap_or(0),
            Movement::NextGrapheme => self
                .text
                .grapheme_indices(true)
                .find_map(|(i, _)| (i > caret).then_some(i))
                .unwrap_or(self.text.len()),
            Movement::PreviousWord => self
                .text
                .split_word_bound_indices()
                .rev()
                .find_map(|(i, word)| (i < caret && !word.trim().is_empty()).then_some(i))
                .unwrap_or(0),
            Movement::NextWord => self
                .text
                .split_word_bound_indices()
                .find_map(|(i, word)| {
                    (i + word.len() > caret && !word.trim().is_empty()).then_some(i + word.len())
                })
                .unwrap_or(self.text.len()),
            Movement::LineStart => self.text[..caret].rfind('\n').map_or(0, |i| i + 1),
            Movement::LineEnd => {
                let end = self.text[caret..]
                    .find('\n')
                    .map_or(self.text.len(), |i| caret + i);
                if end > 0
                    && self.text.as_bytes().get(end) == Some(&b'\n')
                    && self.text.as_bytes()[end - 1] == b'\r'
                {
                    end - 1
                } else {
                    end
                }
            }
            Movement::Start => 0,
            Movement::End => self.text.len(),
        }
    }
}

fn check_range(text: &str, range: &Range<usize>) -> Result<(), EditError> {
    if range.start > range.end
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        Err(EditError::Boundary)
    } else {
        Ok(())
    }
}

/// Convert a native UTF-16 offset. A position inside a surrogate pair refuses.
pub fn utf16_to_utf8(text: &str, offset: usize) -> Result<usize, EditError> {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += character.len_utf16();
        if units > offset {
            return Err(EditError::Boundary);
        }
    }
    if units == offset {
        Ok(text.len())
    } else {
        Err(EditError::Boundary)
    }
}

/// Convert a Rust byte offset to a native UTF-16 position without rounding.
pub fn utf8_to_utf16(text: &str, offset: usize) -> Result<usize, EditError> {
    if !text.is_char_boundary(offset) {
        return Err(EditError::Boundary);
    }
    Ok(text[..offset].encode_utf16().count())
}

#[cfg(test)]
mod tests;
