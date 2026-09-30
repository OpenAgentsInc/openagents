//! Local composer input and revision-bound submission for desktop adapters.
//!
//! The editor survives newer versions of the same semantic composer. A new
//! surface, node, or input token starts a new editing lifetime. This follows
//! Zeron's retained `ComposerInput` design through Rust Native's existing
//! `Composer` contract. Window input, clipboard access, and painting are
//! separate; this module does not send a message or decide its meaning.

pub mod field;

use rust_native::edit::{EditError, EditKind, Editor, Movement, Selection};
use rust_native::{Activation, Element, InputError, Node, ValidatedView};
use std::fmt;

/// A callback's editing lifetime and sequence, with no draft text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub instance: String,
    pub node: String,
    pub token: String,
    pub view_revision: u64,
    pub edit_revision: u64,
    epoch: u64,
}

/// Commands translated from native keys, pointer selection, or IME events.
pub enum Input<'a> {
    Text(&'a str),
    Paste(&'a str),
    Backspace,
    Delete,
    Move { movement: Movement, extend: bool },
    Select(Selection),
    SelectAll,
    Preedit { text: &'a str, selection: Selection },
    Commit(&'a str),
    CancelComposition,
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerError {
    Missing,
    Stale,
    BoundChanged,
    Disabled,
    Busy,
    Empty,
    NotStop,
    EpochExhausted,
    Edit(EditError),
    Input(InputError),
}

impl fmt::Display for ComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("the view does not contain this composer"),
            Self::Stale => f.write_str("composer input does not name the current editing state"),
            Self::BoundChanged => {
                f.write_str("a changed composer byte bound requires a new input token")
            }
            Self::Disabled => f.write_str("the composer is disabled"),
            Self::Busy => f.write_str("the composer offers stop instead of send"),
            Self::Empty => f.write_str("the message is empty"),
            Self::NotStop => f.write_str("the composer has no current stop action"),
            Self::EpochExhausted => f.write_str("composer editing lifetime is exhausted"),
            Self::Edit(error) => error.fmt(f),
            Self::Input(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ComposerError {}

impl From<EditError> for ComposerError {
    fn from(error: EditError) -> Self {
        Self::Edit(error)
    }
}

/// A validated input answer. The application still checks domain authority
/// and assigns a durable message ID before sending it.
pub struct Submission {
    pub token: String,
    pub text: String,
    pub stamp: Stamp,
}

impl fmt::Debug for Submission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Submission")
            .field("token", &self.token)
            .field("bytes", &self.text.len())
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}

/// Whether mounting changes the control's editing lifetime and requests focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mount {
    pub replaced: bool,
    pub focus: bool,
}

struct Mounted {
    instance: String,
    node: String,
    token: String,
    view_revision: u64,
    enabled: bool,
    editor: Editor,
}

/// Draft reconciliation and input checks, usable without a window.
#[derive(Default)]
pub struct ComposerDraft {
    mounted: Option<Mounted>,
    epoch: u64,
}

impl ComposerDraft {
    /// Mount the application's current validated view. A rerender with the
    /// same token never replaces text, selection, undo, or marked input.
    pub fn mount<I>(
        &mut self,
        view: &ValidatedView<I>,
        node: &str,
    ) -> Result<Mount, ComposerError> {
        let Element::Composer {
            token,
            max_bytes,
            enabled,
            draft,
            focus,
            ..
        } = &find(view, node)?.element
        else {
            return Err(ComposerError::Missing);
        };
        let current = view.view();
        if let Some(mounted) = &mut self.mounted {
            if mounted.instance == current.instance && current.revision < mounted.view_revision {
                return Err(ComposerError::Stale);
            }
            if mounted.instance == current.instance
                && mounted.node == node
                && mounted.token == *token
            {
                if mounted.editor.max_bytes() != *max_bytes {
                    return Err(ComposerError::BoundChanged);
                }
                mounted.view_revision = current.revision;
                mounted.enabled = *enabled;
                return Ok(Mount {
                    replaced: false,
                    focus: false,
                });
            }
        }
        let editor = Editor::new(draft.as_deref().unwrap_or_default(), *max_bytes)?;
        self.advance_epoch()?;
        self.mounted = Some(Mounted {
            instance: current.instance.clone(),
            node: node.into(),
            token: token.clone(),
            view_revision: current.revision,
            enabled: *enabled,
            editor,
        });
        Ok(Mount {
            replaced: true,
            focus: *focus && *enabled,
        })
    }

    /// End the lifetime. Even a later mount with identical identifiers cannot
    /// accept a callback from the disposed control.
    pub fn dispose(&mut self) {
        self.mounted = None;
    }

    pub fn editor(&self) -> Option<&Editor> {
        self.mounted.as_ref().map(|m| &m.editor)
    }

    pub fn stamp(&self) -> Result<Stamp, ComposerError> {
        let mounted = self.mounted.as_ref().ok_or(ComposerError::Missing)?;
        Ok(Stamp {
            instance: mounted.instance.clone(),
            node: mounted.node.clone(),
            token: mounted.token.clone(),
            view_revision: mounted.view_revision,
            edit_revision: mounted.editor.revision(),
            epoch: self.epoch,
        })
    }

    pub fn apply(
        &mut self,
        stamp: &Stamp,
        input: Input<'_>,
        at_ms: u64,
    ) -> Result<(), ComposerError> {
        self.check(stamp)?;
        let mounted = self.mounted.as_mut().ok_or(ComposerError::Missing)?;
        if !mounted.enabled
            && !matches!(
                input,
                Input::Move { .. } | Input::Select(_) | Input::SelectAll | Input::CancelComposition
            )
        {
            return Err(ComposerError::Disabled);
        }
        let editor = &mut mounted.editor;
        match input {
            Input::Text(text) => editor.replace(None, text, EditKind::Typing, at_ms)?,
            Input::Paste(text) => editor.replace(None, text, EditKind::Replace, at_ms)?,
            Input::Backspace => editor.delete(true, at_ms)?,
            Input::Delete => editor.delete(false, at_ms)?,
            Input::Move { movement, extend } => editor.move_caret(movement, extend)?,
            Input::Select(selection) => editor.select(selection)?,
            Input::SelectAll => editor.select_all()?,
            Input::Preedit { text, selection } => editor.preedit(text, selection)?,
            Input::Commit(text) => editor.commit(text)?,
            Input::CancelComposition => {
                editor.cancel_composition()?;
            }
            Input::Undo => {
                editor.undo()?;
            }
            Input::Redo => {
                editor.redo()?;
            }
        }
        Ok(())
    }

    /// Prepare the primary send or a choice from this composer. Busy primary
    /// sends refuse; the caller uses `stop` for that action instead.
    pub fn submission<I>(
        &self,
        view: &ValidatedView<I>,
        stamp: &Stamp,
        choice: Option<&str>,
    ) -> Result<Submission, ComposerError> {
        self.check(stamp)?;
        let node = self.current(view)?;
        let Element::Composer {
            token,
            enabled,
            busy,
            choices,
            ..
        } = &node.element
        else {
            return Err(ComposerError::Missing);
        };
        if !enabled {
            return Err(ComposerError::Disabled);
        }
        let answer = match choice {
            Some(choice) if choices.iter().any(|c| c.token == choice) => choice,
            Some(_) => return Err(ComposerError::Stale),
            None if *busy => return Err(ComposerError::Busy),
            None => token,
        };
        let editor = self.editor().ok_or(ComposerError::Missing)?;
        if editor.is_composing() {
            return Err(EditError::Composing.into());
        }
        if editor.text().trim().is_empty() {
            return Err(ComposerError::Empty);
        }
        view.accept_composer(answer, editor.text())
            .map_err(ComposerError::Input)?;
        Ok(Submission {
            token: answer.into(),
            text: editor.text().into(),
            stamp: stamp.clone(),
        })
    }

    /// Resolve stop through the current view's existing typed activation.
    pub fn stop<'a, I>(
        &self,
        view: &'a ValidatedView<I>,
        stamp: &Stamp,
    ) -> Result<&'a I, ComposerError> {
        self.check(stamp)?;
        let Element::Composer {
            enabled,
            busy,
            stop,
            ..
        } = &self.current(view)?.element
        else {
            return Err(ComposerError::Missing);
        };
        if !enabled {
            return Err(ComposerError::Disabled);
        }
        if !busy || stop.is_none() {
            return Err(ComposerError::NotStop);
        }
        view.activate(&Activation {
            instance: stamp.instance.clone(),
            revision: stamp.view_revision,
            node: stamp.node.clone(),
        })
        .map_err(|_| ComposerError::Stale)
    }

    /// Clear only the exact submitted draft after the application accepts it.
    /// A reply to an older submission never discards text typed afterwards.
    pub fn accepted(&mut self, submission: &Submission) -> Result<bool, ComposerError> {
        let Ok(current) = self.stamp() else {
            return Ok(false);
        };
        let submitted = &submission.stamp;
        if current.instance != submitted.instance
            || current.node != submitted.node
            || current.token != submitted.token
            || current.epoch != submitted.epoch
            || current.edit_revision != submitted.edit_revision
        {
            return Ok(false);
        }
        let mounted = self.mounted.as_ref().ok_or(ComposerError::Missing)?;
        if mounted.editor.text() != submission.text {
            return Ok(false);
        }
        let editor = Editor::new("", mounted.editor.max_bytes())?;
        self.advance_epoch()?;
        self.mounted.as_mut().ok_or(ComposerError::Missing)?.editor = editor;
        Ok(true)
    }

    fn check(&self, stamp: &Stamp) -> Result<(), ComposerError> {
        if *stamp == self.stamp()? {
            Ok(())
        } else {
            Err(ComposerError::Stale)
        }
    }

    fn current<'a, I>(&self, view: &'a ValidatedView<I>) -> Result<&'a Node<I>, ComposerError> {
        let stamp = self.stamp()?;
        if view.view().instance != stamp.instance || view.view().revision != stamp.view_revision {
            return Err(ComposerError::Stale);
        }
        let node = find(view, &stamp.node)?;
        match &node.element {
            Element::Composer {
                token, max_bytes, ..
            } if *token == stamp.token
                && Some(*max_bytes) == self.editor().map(Editor::max_bytes) =>
            {
                Ok(node)
            }
            _ => Err(ComposerError::Stale),
        }
    }

    fn advance_epoch(&mut self) -> Result<(), ComposerError> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or(ComposerError::EpochExhausted)?;
        Ok(())
    }
}

fn find<'a, I>(view: &'a ValidatedView<I>, key: &str) -> Result<&'a Node<I>, ComposerError> {
    let mut pending = vec![&view.view().root];
    while let Some(node) = pending.pop() {
        if node.key == key {
            return Ok(node);
        }
        if let Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. } = &node.element
        {
            pending.extend(children);
        }
    }
    Err(ComposerError::Missing)
}

#[cfg(test)]
mod tests;
