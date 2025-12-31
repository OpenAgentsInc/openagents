use crate::{Caret, Editor, Position};

#[test]
fn test_insert_and_delete() {
    let mut editor = Editor::new("hello");
    editor.set_cursor(Position { line: 0, column: 5 });
    editor.insert_text("!");
    assert_eq!(editor.text(), "hello!");
    editor.delete_backward();
    assert_eq!(editor.text(), "hello");
}

#[test]
fn test_selection_replacement() {
    let mut editor = Editor::new("hello");
    let mut cursor = Caret::new(Position { line: 0, column: 1 });
    cursor.position = Position { line: 0, column: 4 };
    editor.set_cursors(vec![cursor]);
    editor.insert_text("i");
    assert_eq!(editor.text(), "hio");
}

#[test]
fn test_multi_cursor_insert() {
    let mut editor = Editor::new("a\nb\nc");
    editor.set_cursor(Position { line: 0, column: 1 });
    editor.add_cursor(Position { line: 2, column: 1 });
    editor.insert_text("!");
    assert_eq!(editor.text(), "a!\nb\nc!");
}

#[test]
fn test_undo_redo() {
    let mut editor = Editor::new("one");
    editor.set_cursor(Position { line: 0, column: 3 });
    editor.insert_text(" two");
    assert_eq!(editor.text(), "one two");
    editor.undo();
    assert_eq!(editor.text(), "one");
    editor.redo();
    assert_eq!(editor.text(), "one two");
}
