use super::*;

fn editor(text: &str) -> Editor {
    Editor::new(text, 1024).unwrap()
}

#[test]
fn native_offsets_round_trip_every_character_boundary() {
    let text = "a🧑🏽‍💻e\u{301}日本\r\n";
    for byte in 0..=text.len() {
        if text.is_char_boundary(byte) {
            let native = utf8_to_utf16(text, byte).unwrap();
            assert_eq!(utf16_to_utf8(text, native), Ok(byte));
        } else {
            assert_eq!(utf8_to_utf16(text, byte), Err(EditError::Boundary));
        }
    }
    assert_eq!(utf16_to_utf8("a😀", 2), Err(EditError::Boundary));
    assert_eq!(utf16_to_utf8(text, 1000), Err(EditError::Boundary));
}

#[test]
fn movement_and_deletion_preserve_emoji_combining_sequences_and_crlf() {
    for grapheme in ["👨‍👩‍👧‍👦", "e\u{301}", "🇯🇵", "🧑🏽‍💻", "\r\n"]
    {
        let text = format!("a{grapheme}z");
        let mut draft = editor(&text);
        draft.move_caret(Movement::PreviousGrapheme, false).unwrap();
        assert_eq!(draft.selection().caret, 1 + grapheme.len());
        draft.delete(true, 0).unwrap();
        assert_eq!(draft.text(), "az");
        draft.undo().unwrap();
        draft.select(Selection::collapsed(1)).unwrap();
        draft.delete(false, 1000).unwrap();
        assert_eq!(draft.text(), "az");
        draft.undo().unwrap();
        draft.move_caret(Movement::NextGrapheme, false).unwrap();
        assert_eq!(draft.selection().caret, 1 + grapheme.len());
    }
}

#[test]
fn a_native_caret_inside_a_grapheme_does_not_split_keyboard_deletion() {
    for backwards in [false, true] {
        let mut draft = editor("ae\u{301}z");
        draft.select(Selection::collapsed(2)).unwrap();
        draft.delete(backwards, 0).unwrap();
        assert_eq!(draft.text(), "az");
    }
}

#[test]
fn extension_retains_the_anchor_when_selection_reverses() {
    let mut draft = editor("abcd");
    draft.select(Selection::collapsed(2)).unwrap();
    draft.move_caret(Movement::PreviousGrapheme, true).unwrap();
    assert_eq!(
        draft.selection(),
        Selection {
            anchor: 2,
            caret: 1
        }
    );
    assert_eq!(draft.selected_text(), "b");
    draft.move_caret(Movement::NextGrapheme, true).unwrap();
    draft.move_caret(Movement::NextGrapheme, true).unwrap();
    assert_eq!(
        draft.selection(),
        Selection {
            anchor: 2,
            caret: 3
        }
    );
    draft.move_caret(Movement::PreviousGrapheme, false).unwrap();
    assert_eq!(draft.selection(), Selection::collapsed(2));
}

#[test]
fn word_and_logical_line_navigation_handle_crlf() {
    let mut draft = editor("one two\r\n日本 three");
    draft.move_caret(Movement::Start, false).unwrap();
    draft.move_caret(Movement::NextWord, false).unwrap();
    assert_eq!(draft.selection().caret, 3);
    draft.move_caret(Movement::NextWord, false).unwrap();
    assert_eq!(draft.selection().caret, 7);
    draft.move_caret(Movement::LineEnd, false).unwrap();
    assert_eq!(draft.selection().caret, 7);
    draft.move_caret(Movement::NextGrapheme, false).unwrap();
    assert_eq!(draft.selection().caret, 9);
    draft.move_caret(Movement::End, false).unwrap();
    draft.move_caret(Movement::LineStart, false).unwrap();
    assert_eq!(draft.selection().caret, 9);
    draft.move_caret(Movement::PreviousWord, false).unwrap();
    assert_eq!(draft.selection().caret, 4);
}

#[test]
fn typing_merges_until_a_pause_or_caret_move() {
    let mut draft = editor("");
    for (at, text) in [(0, "h"), (100, "i"), (200, "!")] {
        draft.replace(None, text, EditKind::Typing, at).unwrap();
    }
    draft.replace(None, "x", EditKind::Typing, 1000).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "hi!");
    draft.undo().unwrap();
    assert_eq!(draft.text(), "");
    draft.redo().unwrap();
    draft.move_caret(Movement::Start, false).unwrap();
    draft.replace(None, "a", EditKind::Typing, 1100).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "hi!");
    assert_eq!(draft.selection(), Selection::collapsed(0));
}

#[test]
fn multiline_paste_and_following_typing_are_separate_undo_steps() {
    let mut draft = editor("before ");
    draft
        .replace(None, "one\ntwo", EditKind::Replace, 0)
        .unwrap();
    draft.replace(None, "!", EditKind::Typing, 10).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "before one\ntwo");
    draft.undo().unwrap();
    assert_eq!(draft.text(), "before ");
}

#[test]
fn contiguous_deletes_coalesce_but_opposite_directions_do_not() {
    let mut draft = editor("abcd");
    draft.delete(true, 0).unwrap();
    draft.delete(true, 100).unwrap();
    assert_eq!(draft.text(), "ab");
    draft.undo().unwrap();
    assert_eq!(draft.text(), "abcd");
    draft.select(Selection::collapsed(2)).unwrap();
    draft.delete(false, 200).unwrap();
    draft.delete(true, 300).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "abd");
    draft.undo().unwrap();
    assert_eq!(draft.text(), "abcd");
}

#[test]
fn a_backwards_clock_breaks_coalescing() {
    let mut draft = editor("");
    draft.replace(None, "a", EditKind::Typing, 100).unwrap();
    draft.replace(None, "b", EditKind::Typing, 99).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "a");
}

#[test]
fn an_ime_composition_replaces_the_selection_and_undoes_as_one_edit() {
    let mut draft = editor("aoldz");
    let selection = Selection {
        anchor: 4,
        caret: 1,
    };
    draft.select(selection).unwrap();
    draft.preedit("に", Selection::collapsed(3)).unwrap();
    assert_eq!(draft.marked_range(), Some(1..4));
    draft.preedit("にほん", Selection::collapsed(9)).unwrap();
    assert_eq!(draft.text(), "aにほんz");
    draft.commit("日本").unwrap();
    assert_eq!(draft.text(), "a日本z");
    assert!(!draft.is_composing());
    draft.undo().unwrap();
    assert_eq!(draft.text(), "aoldz");
    assert_eq!(draft.selection(), selection);
    draft.redo().unwrap();
    assert_eq!(draft.text(), "a日本z");
}

#[test]
fn cancelling_composition_preserves_the_redo_branch() {
    let mut draft = editor("a");
    draft.replace(None, "b", EditKind::Typing, 0).unwrap();
    draft.undo().unwrap();
    draft.preedit("仮", Selection::collapsed(3)).unwrap();
    draft.cancel_composition().unwrap();
    assert_eq!(draft.text(), "a");
    draft.redo().unwrap();
    assert_eq!(draft.text(), "ab");
}

#[test]
fn empty_preedit_still_retains_the_composition_until_commit_or_cancel() {
    let mut draft = editor("replace");
    draft.select_all().unwrap();
    draft.preedit("", Selection::collapsed(0)).unwrap();
    assert!(draft.is_composing());
    assert_eq!(draft.text(), "");
    draft.cancel_composition().unwrap();
    assert_eq!(draft.text(), "replace");
}

#[test]
fn undo_during_composition_cancels_without_consuming_an_earlier_edit() {
    let mut draft = editor("a");
    draft.replace(None, "b", EditKind::Typing, 0).unwrap();
    draft.preedit("仮", Selection::collapsed(3)).unwrap();
    draft.undo().unwrap();
    assert_eq!(draft.text(), "ab");
    draft.undo().unwrap();
    assert_eq!(draft.text(), "a");
}

#[test]
fn composition_refuses_unrelated_edits_and_invalid_relative_selection() {
    let mut draft = editor("a");
    draft.preedit("仮", Selection::collapsed(3)).unwrap();
    let before = (draft.text().to_owned(), draft.selection(), draft.revision());
    assert_eq!(
        draft.replace(None, "x", EditKind::Replace, 0),
        Err(EditError::Composing)
    );
    assert_eq!(
        draft.move_caret(Movement::Start, false),
        Err(EditError::Composing)
    );
    assert_eq!(
        draft.preedit("仮", Selection::collapsed(2)),
        Err(EditError::Boundary)
    );
    assert_eq!(
        before,
        (draft.text().to_owned(), draft.selection(), draft.revision())
    );
}

#[test]
fn bounds_and_invalid_ranges_refuse_atomically_even_during_composition() {
    assert_eq!(Editor::new("", 0).unwrap_err(), EditError::Bound);
    assert_eq!(Editor::new("", 65537).unwrap_err(), EditError::Bound);
    assert_eq!(Editor::new("😀", 3).unwrap_err(), EditError::TooLong);
    let mut draft = Editor::new("😀", 4).unwrap();
    assert_eq!(
        draft.select(Selection::collapsed(1)),
        Err(EditError::Boundary)
    );
    assert_eq!(
        draft.replace(Some(Range { start: 4, end: 3 }), "", EditKind::Replace, 0),
        Err(EditError::Boundary)
    );
    assert_eq!(
        draft.replace(None, "x", EditKind::Typing, 0),
        Err(EditError::TooLong)
    );
    assert_eq!(draft.revision(), 0);
    draft.select_all().unwrap();
    draft.preedit("日", Selection::collapsed(3)).unwrap();
    let revision = draft.revision();
    assert_eq!(draft.commit("日本"), Err(EditError::TooLong));
    assert_eq!(draft.text(), "日");
    assert!(draft.is_composing());
    assert_eq!(draft.revision(), revision);
    draft.cancel_composition().unwrap();
    assert_eq!(draft.text(), "😀");
}

#[test]
fn editing_after_undo_invalidates_redo() {
    let mut draft = editor("");
    draft.replace(None, "a", EditKind::Typing, 0).unwrap();
    draft.undo().unwrap();
    draft.replace(None, "b", EditKind::Typing, 10).unwrap();
    assert!(!draft.redo().unwrap());
    assert_eq!(draft.text(), "b");
}

#[test]
fn history_is_bounded_by_step_count_and_text_bytes() {
    let mut draft = Editor::new("", 65536).unwrap();
    for i in 0..1000 {
        draft.select_all().unwrap();
        let value = if i % 2 == 0 { "x" } else { "y" }.repeat(65536);
        draft.replace(None, &value, EditKind::Replace, i).unwrap();
        assert!(draft.undo.entries.len() <= HISTORY_STEPS);
        assert!(draft.undo.bytes <= HISTORY_BYTES);
    }
    let mut small = editor("");
    for i in 0..1000 {
        small.select_all().unwrap();
        small.replace(None, "x", EditKind::Replace, i).unwrap();
    }
    assert_eq!(small.undo.entries.len(), HISTORY_STEPS);
}

#[test]
fn exhausted_revisions_refuse_without_changing_text_or_history() {
    let mut draft = editor("a");
    draft.revision = u64::MAX;
    assert_eq!(
        draft.replace(None, "b", EditKind::Typing, 0),
        Err(EditError::RevisionExhausted)
    );
    assert_eq!(draft.text(), "a");
    assert!(draft.undo.entries.is_empty());
}

#[test]
fn debug_output_does_not_disclose_the_draft() {
    let draft = editor("private words");
    assert!(!format!("{draft:?}").contains("private words"));
}
