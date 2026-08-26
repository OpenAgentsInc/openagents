//! Key chords to edit commands.
//!
//! # Provenance
//!
//! Ported from `crates/codegen/xai-ratatui-textarea/src/editor_keys.rs` in
//! grok-build, © 2023-2026 SpaceXAI, Apache-2.0, read at commit
//! `07b2f7144fd5c5c9d3dd1966937a87852d2dbdb8`. See `LICENSE-APACHE-xai`.
//!
//! `classify_key_event` is the upstream function, arm for arm and in the same
//! order — the order carries meaning, since several arms deliberately shadow
//! later ones. `is_altgr` is inlined from the upstream crate root, and
//! `resolve_movement` is not ported: it resolves the vertical keys against
//! wrap geometry that the upstream widget owns, and `super::Composer` answers
//! those keys against its own wrap rows instead.
//!
//! The comments on the odd-looking arms are upstream's and are worth keeping:
//! they record terminal behaviour that is not guessable.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::edit::{EditCommand, WordStyle};

// On Windows, AltGr arrives as Ctrl+Alt; on other platforms it's composed
// before reaching us.
#[cfg(target_os = "windows")]
#[inline]
fn is_altgr(modifiers: KeyModifiers) -> bool {
    let without_shift = modifiers & !KeyModifiers::SHIFT;
    without_shift == (KeyModifiers::CONTROL | KeyModifiers::ALT)
}

#[cfg(not(target_os = "windows"))]
#[inline]
fn is_altgr(_modifiers: KeyModifiers) -> bool {
    false
}

pub fn classify_key_event(event: &KeyEvent) -> Option<EditCommand> {
    match event {
        // Some terminals encode Ctrl-B/Ctrl-F as bare C0 characters.
        KeyEvent {
            code: KeyCode::Char('\u{0002}'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditCommand::MoveGraphemeLeft),
        KeyEvent {
            code: KeyCode::Char('\u{0006}'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(EditCommand::MoveGraphemeRight),
        KeyEvent {
            code: KeyCode::Char('h'),
            modifiers,
            ..
        } if *modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            Some(EditCommand::DeleteWordBackward(WordStyle::Small))
        }
        // Kitty protocol loss can surface Backspace as raw BS or DEL;
        // modifiers are unreliable.
        KeyEvent {
            code: KeyCode::Char('\u{0008}') | KeyCode::Char('\u{007f}'),
            ..
        } => Some(EditCommand::DeleteGraphemeBackward),
        KeyEvent {
            code: KeyCode::Backspace,
            modifiers,
            ..
        } => Some(backspace_command(*modifiers)),
        KeyEvent {
            code: KeyCode::Delete,
            modifiers,
            ..
        } => Some(delete_command(*modifiers)),
        KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::DeleteWordBackward(
            WordStyle::WhitespaceDelimited,
        )),
        KeyEvent {
            code: KeyCode::Left,
            modifiers,
            ..
        } if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) => {
            Some(EditCommand::MoveWordLeft(WordStyle::Small))
        }
        KeyEvent {
            code: KeyCode::Right,
            modifiers,
            ..
        } if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) => {
            Some(EditCommand::MoveWordRight(WordStyle::Small))
        }
        KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::MoveLogicalLineStart),
        KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::MoveLogicalLineEnd),
        KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::MoveGraphemeLeft),
        KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::MoveGraphemeRight),
        KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::ALT,
            ..
        } => Some(EditCommand::MoveWordLeft(WordStyle::Small)),
        KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::ALT,
            ..
        } => Some(EditCommand::MoveWordRight(WordStyle::Small)),
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::DeleteToLineStart),
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::DeleteToLineEnd),
        KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::DeleteGraphemeBackward),
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(EditCommand::DeleteGraphemeForward),
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers,
            ..
        } if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SUPER) => {
            Some(EditCommand::DeleteWordForward(WordStyle::Small))
        }
        KeyEvent {
            code: KeyCode::Char(character),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            ..
        } if !character.is_control() => {
            let character = if event.modifiers.contains(KeyModifiers::SHIFT) {
                shifted_char(*character)
            } else {
                *character
            };
            Some(EditCommand::Insert(character))
        }
        KeyEvent {
            code: KeyCode::Char(character),
            modifiers,
            ..
        } if is_altgr(*modifiers) && !character.is_control() => {
            Some(EditCommand::Insert(*character))
        }
        _ => None,
    }
}

fn shifted_char(character: char) -> char {
    if character.is_ascii_lowercase() {
        character.to_ascii_uppercase()
    } else {
        character
    }
}

fn backspace_command(modifiers: KeyModifiers) -> EditCommand {
    // Backspace preserves exact historical chords; extra modifiers fall back
    // to grapheme delete.
    match modifiers {
        KeyModifiers::ALT | KeyModifiers::CONTROL => {
            EditCommand::DeleteWordBackward(WordStyle::Small)
        }
        KeyModifiers::SUPER => EditCommand::DeleteToLineStart,
        _ => EditCommand::DeleteGraphemeBackward,
    }
}

fn delete_command(modifiers: KeyModifiers) -> EditCommand {
    // Delete accepts Shift in addition to a word modifier because enhanced
    // protocols retain it.
    if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL | KeyModifiers::SUPER) {
        EditCommand::DeleteWordForward(WordStyle::Small)
    } else {
        EditCommand::DeleteGraphemeForward
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn a_plain_letter_inserts_itself() {
        assert_eq!(
            classify_key_event(&key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(EditCommand::Insert('a'))
        );
    }

    #[test]
    fn shift_uppercases_an_ascii_letter() {
        assert_eq!(
            classify_key_event(&key(KeyCode::Char('a'), KeyModifiers::SHIFT)),
            Some(EditCommand::Insert('A'))
        );
    }

    #[test]
    fn a_raw_del_byte_is_still_a_backspace() {
        assert_eq!(
            classify_key_event(&key(KeyCode::Char('\u{007f}'), KeyModifiers::NONE)),
            Some(EditCommand::DeleteGraphemeBackward)
        );
    }

    #[test]
    fn ctrl_w_and_alt_backspace_use_different_word_styles() {
        assert_eq!(
            classify_key_event(&key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(EditCommand::DeleteWordBackward(
                WordStyle::WhitespaceDelimited
            ))
        );
        assert_eq!(
            classify_key_event(&key(KeyCode::Backspace, KeyModifiers::ALT)),
            Some(EditCommand::DeleteWordBackward(WordStyle::Small))
        );
    }

    #[test]
    fn enter_and_escape_are_not_edit_commands() {
        assert_eq!(
            classify_key_event(&key(KeyCode::Enter, KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            classify_key_event(&key(KeyCode::Esc, KeyModifiers::NONE)),
            None
        );
    }
}
