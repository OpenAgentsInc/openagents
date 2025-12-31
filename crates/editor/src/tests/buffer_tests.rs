use crate::{Position, TextBuffer};

#[test]
fn test_line_text_strips_newlines() {
    let buffer = TextBuffer::new("alpha\nbeta\r\ngamma");
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line_text(0), "alpha");
    assert_eq!(buffer.line_text(1), "beta");
    assert_eq!(buffer.line_text(2), "gamma");
}

#[test]
fn test_position_char_roundtrip() {
    let buffer = TextBuffer::new("hello\nworld");
    let pos = Position { line: 1, column: 3 };
    let idx = buffer.position_to_char(pos);
    assert_eq!(idx, 9);
    let round = buffer.char_to_position(idx);
    assert_eq!(round, pos);
}

#[test]
fn test_position_clamps_to_line() {
    let buffer = TextBuffer::new("hi\nthere");
    let pos = Position { line: 0, column: 99 };
    let idx = buffer.position_to_char(pos);
    assert_eq!(idx, 2);
    let round = buffer.char_to_position(idx);
    assert_eq!(round, Position { line: 0, column: 2 });
}
