//! A platform text field's draft, edited through the shared [`Editor`].
//!
//! Phone adapters keep their native field (`UITextView`, `EditText`) for
//! drawing, the caret, dictation, and the system keyboard. Each change the
//! field makes is reported here as the field's whole text, its selection,
//! and its marked (IME) range, in UTF-16. The mirror turns it into one
//! editor operation (a replacement, a preedit, or a commit), so undo,
//! redo, composition, and deletion follow the same rules as the desktop:
//! a deletion that would split a grapheme removes the whole grapheme, an IME
//! composition is one undo step, and the byte bound refuses text past it.
//! The adapter then shows the returned state, which is the canonical draft.
//!
//! Every call names a [`Stamp`]: the composer's input token, the mirror's
//! editing lifetime, and the editor's revision. A call with any other stamp
//! is stale and changes nothing; the refusal carries the current state so
//! the field can resynchronize. A new token starts a new lifetime and
//! history, and a rerender with the same token keeps them. Refusals never
//! contain draft text.

use super::{EditError, EditKind, Editor, Selection, utf8_to_utf16, utf16_to_utf8};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifies one state of one editing lifetime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub token: String,
    pub lifetime: u64,
    pub revision: u64,
}

/// What the adapter shows: the canonical draft in UTF-16 positions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct State {
    pub stamp: Stamp,
    pub text: String,
    /// Anchor and caret, in UTF-16 code units.
    pub selection: [usize; 2],
    /// The IME's marked range, in UTF-16 code units.
    pub marked: Option<[usize; 2]>,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// A native field's report or command. Positions are UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// The field's text, selection, and marked range after it changed.
    Sync {
        text: String,
        selection: [usize; 2],
        #[serde(default)]
        marked: Option<[usize; 2]>,
        at_ms: u64,
    },
    /// The person moved the caret or selection.
    Select {
        selection: [usize; 2],
    },
    /// The delete key: the selection, or one whole grapheme.
    Delete {
        backwards: bool,
        at_ms: u64,
    },
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorError {
    /// The stamp does not name the current lifetime and revision.
    Stale,
    /// Nothing is mounted.
    Missing,
    Edit(EditError),
}

impl fmt::Display for MirrorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale => f.write_str("the edit does not name the current draft"),
            Self::Missing => f.write_str("no composer is mounted"),
            Self::Edit(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for MirrorError {}

impl From<EditError> for MirrorError {
    fn from(error: EditError) -> Self {
        Self::Edit(error)
    }
}

impl MirrorError {
    /// A short, stable name for adapters.
    pub fn code(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::Missing => "missing",
            Self::Edit(EditError::Bound) => "bound",
            Self::Edit(EditError::TooLong) => "too_long",
            Self::Edit(EditError::Boundary) => "boundary",
            Self::Edit(EditError::Composing) => "composing",
            Self::Edit(EditError::RevisionExhausted) => "exhausted",
        }
    }
}

struct Field {
    token: String,
    editor: Editor,
}

/// One composer's draft for one native field.
#[derive(Default)]
pub struct Mirror {
    field: Option<Field>,
    lifetime: u64,
}

impl Mirror {
    /// Mount the view's composer. The same token keeps the draft, its
    /// selection, history, and composition; the byte bound must not change
    /// without a new token. A new token starts a new lifetime with `draft`.
    /// Returns the state and whether the draft was replaced.
    pub fn mount(
        &mut self,
        token: &str,
        max_bytes: usize,
        draft: Option<&str>,
    ) -> Result<(State, bool), MirrorError> {
        if let Some(field) = &self.field
            && field.token == token
        {
            if field.editor.max_bytes() != max_bytes {
                return Err(MirrorError::Stale);
            }
            return Ok((self.state().ok_or(MirrorError::Missing)?, false));
        }
        let editor = Editor::new(draft.unwrap_or_default(), max_bytes)?;
        self.lifetime = self
            .lifetime
            .checked_add(1)
            .ok_or(EditError::RevisionExhausted)?;
        self.field = Some(Field {
            token: token.into(),
            editor,
        });
        Ok((self.state().ok_or(MirrorError::Missing)?, true))
    }

    /// End the lifetime; later calls with its stamps are stale.
    pub fn dispose(&mut self) {
        self.field = None;
    }

    pub fn editor(&self) -> Option<&Editor> {
        self.field.as_ref().map(|field| &field.editor)
    }

    pub fn stamp(&self) -> Option<Stamp> {
        self.field.as_ref().map(|field| Stamp {
            token: field.token.clone(),
            lifetime: self.lifetime,
            revision: field.editor.revision(),
        })
    }

    pub fn state(&self) -> Option<State> {
        let field = self.field.as_ref()?;
        let editor = &field.editor;
        let text = editor.text();
        let at = |offset| utf8_to_utf16(text, offset).unwrap_or(0);
        let selection = editor.selection();
        Some(State {
            stamp: self.stamp()?,
            text: text.into(),
            selection: [at(selection.anchor), at(selection.caret)],
            marked: editor.marked_range().map(|m| [at(m.start), at(m.end)]),
            can_undo: editor.can_undo(),
            can_redo: editor.can_redo(),
        })
    }

    /// Apply one change named by `stamp`. A refused change leaves the draft
    /// as it was; the caller shows [`Mirror::state`] again.
    pub fn apply(&mut self, stamp: &Stamp, change: Change) -> Result<State, MirrorError> {
        if self.stamp().as_ref() != Some(stamp) {
            return Err(if self.field.is_none() {
                MirrorError::Missing
            } else {
                MirrorError::Stale
            });
        }
        let editor = &mut self.field.as_mut().ok_or(MirrorError::Missing)?.editor;
        match change {
            Change::Sync {
                text,
                selection,
                marked,
                at_ms,
            } => sync(editor, &text, selection, marked, at_ms)?,
            Change::Select { selection } => {
                let text = editor.text();
                let selection = Selection {
                    anchor: utf16_to_utf8(text, selection[0])?,
                    caret: utf16_to_utf8(text, selection[1])?,
                };
                if editor.selection() != selection {
                    editor.select(selection)?;
                }
            }
            Change::Delete { backwards, at_ms } => editor.delete(backwards, at_ms)?,
            Change::Undo => {
                editor.undo()?;
            }
            Change::Redo => {
                editor.redo()?;
            }
        }
        self.state().ok_or(MirrorError::Missing)
    }

    /// The application accepted `text` sent from the lifetime of `stamp`:
    /// clear the draft only if it is still exactly that text, so a reply
    /// never discards what was typed after the send. Returns whether the
    /// draft cleared.
    pub fn submitted(&mut self, stamp: &Stamp, text: &str) -> Result<bool, MirrorError> {
        let Some(field) = &self.field else {
            return Ok(false);
        };
        if field.token != stamp.token
            || self.lifetime != stamp.lifetime
            || field.editor.revision() != stamp.revision
            || field.editor.text() != text
        {
            return Ok(false);
        }
        let editor = Editor::new("", field.editor.max_bytes())?;
        self.lifetime = self
            .lifetime
            .checked_add(1)
            .ok_or(EditError::RevisionExhausted)?;
        if let Some(field) = &mut self.field {
            field.editor = editor;
        }
        Ok(true)
    }
}

/// A JSON request to a mirror, for adapters across a C or JNI boundary.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Mount {
        token: String,
        max_bytes: usize,
        #[serde(default)]
        draft: Option<String>,
    },
    Apply {
        stamp: Stamp,
        change: Change,
    },
    Submitted {
        stamp: Stamp,
        text: String,
    },
    Dispose,
}

/// The most bytes one JSON request may carry: a full draft and its framing.
pub const MAX_REQUEST_BYTES: usize = crate::input::MAX_INPUT_VALUE_BYTES * 7 + 4096;

impl Mirror {
    /// Answers one JSON request: `mount` (`token`, `max_bytes`, `draft`),
    /// `apply` (`stamp`, `change`), `submitted` (`stamp`, `text`), or
    /// `dispose`. The reply is `{"state": ..., "replaced"?, "cleared"?}` or,
    /// for a refusal, `{"error": code, "state": ...}` with the current state.
    pub fn call_json(&mut self, bytes: &[u8]) -> Vec<u8> {
        let reply = if bytes.len() > MAX_REQUEST_BYTES {
            serde_json::json!({"error": "too_long", "state": self.state()})
        } else {
            match serde_json::from_slice::<Request>(bytes) {
                Err(_) => serde_json::json!({"error": "unreadable", "state": self.state()}),
                Ok(Request::Mount {
                    token,
                    max_bytes,
                    draft,
                }) => match self.mount(&token, max_bytes, draft.as_deref()) {
                    Ok((state, replaced)) => {
                        serde_json::json!({"state": state, "replaced": replaced})
                    }
                    Err(error) => serde_json::json!({"error": error.code(), "state": self.state()}),
                },
                Ok(Request::Apply { stamp, change }) => match self.apply(&stamp, change) {
                    Ok(state) => serde_json::json!({"state": state}),
                    Err(error) => serde_json::json!({"error": error.code(), "state": self.state()}),
                },
                Ok(Request::Submitted { stamp, text }) => match self.submitted(&stamp, &text) {
                    Ok(cleared) => serde_json::json!({"state": self.state(), "cleared": cleared}),
                    Err(error) => serde_json::json!({"error": error.code(), "state": self.state()}),
                },
                Ok(Request::Dispose) => {
                    self.dispose();
                    serde_json::json!({"state": null})
                }
            }
        };
        serde_json::to_vec(&reply).unwrap_or_default()
    }
}

/// Turn the field's new state into one editor operation.
fn sync(
    editor: &mut Editor,
    text: &str,
    selection: [usize; 2],
    marked: Option<[usize; 2]>,
    at_ms: u64,
) -> Result<(), EditError> {
    if text.len() > editor.max_bytes() {
        return Err(EditError::TooLong);
    }
    let selection = Selection {
        anchor: utf16_to_utf8(text, selection[0])?,
        caret: utf16_to_utf8(text, selection[1])?,
    };
    let marked = match marked {
        Some([start, end]) if start < end => {
            Some(utf16_to_utf8(text, start)?..utf16_to_utf8(text, end)?)
        }
        _ => None,
    };
    // Relative to the marked text, clamped into it.
    let within = |range: &std::ops::Range<usize>| Selection {
        anchor: selection.anchor.clamp(range.start, range.end) - range.start,
        caret: selection.caret.clamp(range.start, range.end) - range.start,
    };
    if let Some(composing) = editor.marked_range() {
        let old = editor.text();
        let (head, tail) = (&old[..composing.start], &old[composing.end..]);
        let keeps_context =
            text.len() >= head.len() + tail.len() && text.starts_with(head) && text.ends_with(tail);
        match &marked {
            Some(range)
                if keeps_context
                    && range.start == head.len()
                    && range.end == text.len() - tail.len() =>
            {
                return editor.preedit(&text[range.clone()], within(range));
            }
            None if keeps_context => {
                editor.commit(&text[head.len()..text.len() - tail.len()])?;
                return select(editor, text, selection);
            }
            // The field replaced more than its composition: end it and
            // treat the change as a new edit.
            _ => {
                editor.cancel_composition()?;
            }
        }
    }
    let old = editor.text();
    if old == text {
        if let Some(range) = &marked {
            editor.select(Selection {
                anchor: range.start,
                caret: range.end,
            })?;
            return editor.preedit(&text[range.clone()], within(range));
        }
        return select(editor, text, selection);
    }
    let (start, old_end, new_end) = difference(old, text, selection);
    if let Some(range) = &marked
        && range.start <= start
        && range.end >= new_end
        && text.len() - range.end <= old.len() - old_end
    {
        // A composition began: the marked text replaced this old range.
        let replaced = range.start..old.len() - (text.len() - range.end);
        if old.is_char_boundary(replaced.start)
            && old.is_char_boundary(replaced.end)
            && old[..replaced.start] == text[..range.start]
            && old[replaced.end..] == text[range.end..]
        {
            editor.select(Selection {
                anchor: replaced.start,
                caret: replaced.end,
            })?;
            return editor.preedit(&text[range.clone()], within(range));
        }
    }
    let inserted = &text[start..new_end];
    let mut range = start..old_end;
    let kind = if inserted.is_empty() {
        // A deletion never splits a grapheme, whatever the keyboard sent.
        range =
            editor.grapheme_boundary(range.start, false)..editor.grapheme_boundary(range.end, true);
        if selection.caret <= start {
            EditKind::Backspace
        } else {
            EditKind::Delete
        }
    } else if range.is_empty() {
        EditKind::Typing
    } else {
        EditKind::Replace
    };
    editor.replace(Some(range), inserted, kind, at_ms)?;
    select(editor, text, selection)
}

/// Take the field's selection when the draft matches the field's text.
fn select(editor: &mut Editor, text: &str, selection: Selection) -> Result<(), EditError> {
    if editor.text() == text && editor.selection() != selection {
        editor.select(selection)?;
    }
    Ok(())
}

/// The changed region: its start, where it ends in the old text, and where
/// it ends in the new text. A collapsed caret after the change ends the
/// inserted text, so typing "a" at the start of "aa" inserts at the start.
fn difference(old: &str, new: &str, selection: Selection) -> (usize, usize, usize) {
    let shared = old.len().min(new.len());
    let mut limit = shared;
    if selection.anchor == selection.caret {
        limit = limit.min(new.len() - selection.caret.min(new.len()));
    }
    let mut suffix = 0;
    for (a, b) in old.chars().rev().zip(new.chars().rev()) {
        if a != b || suffix + a.len_utf8() > limit {
            break;
        }
        suffix += a.len_utf8();
    }
    let mut prefix = 0;
    for (a, b) in old.chars().zip(new.chars()) {
        if a != b || prefix + a.len_utf8() > shared - suffix {
            break;
        }
        prefix += a.len_utf8();
    }
    (prefix, old.len() - suffix, new.len() - suffix)
}

#[cfg(test)]
mod tests;
