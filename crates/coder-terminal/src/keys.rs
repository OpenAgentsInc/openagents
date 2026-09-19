//! Terminal keys onto editor commands.
//!
//! The mapping is readline where readline exists — `Ctrl-A`/`Ctrl-E` for the
//! line edges, `Ctrl-W`/`Ctrl-K`/`Ctrl-U` for the deletes, `Alt-B`/`Alt-F`
//! for word motion, `Up`/`Down` for wrapped rows and history — and the
//! terminal's own keys where it does not: `Home`, `End`, `Backspace`,
//! `Delete`, arrows. `Enter` submits; `Alt-Enter` and `Ctrl-J` put a newline
//! in the draft instead.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::composer::GUTTER;
use crate::editor::Editor;

/// What a key press did to the composer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerAction {
    /// Nothing happened — the key is not the composer's.
    Ignored,
    /// The caret moved; the box redraws but no draft changed.
    Moved,
    /// The draft changed; the box redraws and its height may too.
    Edited,
    /// `Enter` fired: the submitted draft rides along.
    Submitted(String),
}

/// Applies one terminal key event to `editor`. `width` is the composer's
/// total cell width; the draft's room is `width - GUTTER`.
pub fn handle_key(editor: &mut Editor, width: usize, key: &KeyEvent) -> ComposerAction {
    let inner = width.saturating_sub(GUTTER).max(1);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match (key.code, alt, ctrl) {
        // Enter submits; Alt-Enter and Ctrl-J keep a newline in the draft.
        (KeyCode::Enter, true, false) | (KeyCode::Char('j'), false, true) => {
            editor.newline();
            ComposerAction::Edited
        }
        (KeyCode::Enter, false, false) => ComposerAction::Submitted(editor.take()),

        // Modified characters first — Ctrl-char arrives as Char + CONTROL
        // and Alt-char as Char + ALT on most terminals, and either would
        // fall through to plain insertion otherwise.
        (KeyCode::Char('a'), _, true) => {
            editor.home();
            ComposerAction::Moved
        }
        (KeyCode::Char('e'), _, true) => {
            editor.end();
            ComposerAction::Moved
        }
        (KeyCode::Char('w'), _, true) => {
            editor.kill_word();
            ComposerAction::Edited
        }
        (KeyCode::Char('k'), _, true) => {
            editor.kill_line_end();
            ComposerAction::Edited
        }
        (KeyCode::Char('u'), _, true) => {
            editor.kill_line_start();
            ComposerAction::Edited
        }
        (KeyCode::Char('b'), true, _) | (KeyCode::Char('b'), _, true) => {
            editor.word_back_motion();
            ComposerAction::Moved
        }
        (KeyCode::Char('f'), true, _) | (KeyCode::Char('f'), _, true) => {
            editor.word_forward();
            ComposerAction::Moved
        }
        (KeyCode::Char(ch), false, false) => {
            editor.insert(ch);
            ComposerAction::Edited
        }

        (KeyCode::Backspace, _, _) => {
            editor.backspace();
            ComposerAction::Edited
        }
        (KeyCode::Delete, _, _) => {
            editor.delete();
            ComposerAction::Edited
        }
        (KeyCode::Left, true, _) => {
            editor.word_back_motion();
            ComposerAction::Moved
        }
        (KeyCode::Right, true, _) => {
            editor.word_forward();
            ComposerAction::Moved
        }
        (KeyCode::Left, _, _) => moved(editor.left()),
        (KeyCode::Right, _, _) => moved(editor.right()),
        (KeyCode::Home, _, _) => {
            editor.home();
            ComposerAction::Moved
        }
        (KeyCode::End, _, _) => {
            editor.end();
            ComposerAction::Moved
        }
        // Up and Down walk wrapped rows first; at the edges they walk
        // submitted history instead.
        (KeyCode::Up, _, _) => {
            if editor.up(inner) || editor.previous_history() {
                ComposerAction::Moved
            } else {
                ComposerAction::Ignored
            }
        }
        (KeyCode::Down, _, _) => {
            if editor.down(inner) || editor.next_history() {
                ComposerAction::Moved
            } else {
                ComposerAction::Ignored
            }
        }
        _ => ComposerAction::Ignored,
    }
}

fn moved(did: bool) -> ComposerAction {
    if did {
        ComposerAction::Moved
    } else {
        ComposerAction::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn modified(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn a_plain_character_edits() {
        let mut editor = Editor::new();
        let action = handle_key(&mut editor, 40, &key(KeyCode::Char('x')));
        assert_eq!(action, ComposerAction::Edited);
        assert_eq!(editor.text(), "x");
    }

    #[test]
    fn enter_submits_the_draft() {
        let mut editor = Editor::new();
        editor.insert_str("run it");
        let action = handle_key(&mut editor, 40, &key(KeyCode::Enter));
        assert_eq!(action, ComposerAction::Submitted("run it".to_owned()));
        assert!(editor.is_empty());
    }

    #[test]
    fn alt_enter_keeps_the_draft() {
        let mut editor = Editor::new();
        editor.insert_str("one");
        let action = handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Enter, KeyModifiers::ALT),
        );
        assert_eq!(action, ComposerAction::Edited);
        assert_eq!(editor.text(), "one\n");
    }

    #[test]
    fn ctrl_j_is_a_newline_too() {
        let mut editor = Editor::new();
        let action = handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Char('j'), KeyModifiers::CONTROL),
        );
        assert_eq!(action, ComposerAction::Edited);
        assert_eq!(editor.text(), "\n");
    }

    #[test]
    fn readline_chords_drive_the_editor() {
        let mut editor = Editor::new();
        editor.insert_str("one two");
        handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );
        assert_eq!(editor.caret(), 0);
        handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Char('e'), KeyModifiers::CONTROL),
        );
        assert_eq!(editor.caret(), 7);
        handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert_eq!(editor.text(), "one ");
        handle_key(
            &mut editor,
            40,
            &modified(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert!(editor.is_empty());
    }

    #[test]
    fn up_at_the_top_walks_history() {
        let mut editor = Editor::new();
        editor.insert_str("first");
        editor.take();
        let action = handle_key(&mut editor, 40, &key(KeyCode::Up));
        assert_eq!(action, ComposerAction::Moved);
        assert_eq!(editor.text(), "first");
    }

    #[test]
    fn up_with_nowhere_to_go_is_ignored() {
        let mut editor = Editor::new();
        let action = handle_key(&mut editor, 40, &key(KeyCode::Up));
        assert_eq!(action, ComposerAction::Ignored);
    }
}
