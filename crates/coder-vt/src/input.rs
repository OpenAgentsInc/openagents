//! The bytes a key or a paste sends to the program, as xterm encodes them.

/// A key a client sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    /// Shift-Tab.
    BackTab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// A function key, F1 to F12.
    F(u8),
}

/// Modifier keys held with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
    };
    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        alt: false,
        shift: false,
    };

    fn any(self) -> bool {
        self.ctrl || self.alt || self.shift
    }

    /// xterm's modifier parameter: 1 plus shift 1, alt 2, and control 4.
    fn parameter(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }
}

/// The control character Ctrl sends with `character`, if it has one.
#[must_use]
pub fn control(character: char) -> Option<u8> {
    Some(match character {
        'a'..='z' => character as u8 - b'a' + 1,
        'A'..='Z' => character as u8 - b'A' + 1,
        '@' | ' ' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '/' | '7' => 0x1f,
        '?' | '8' => 0x7f,
        _ => return None,
    })
}

/// Encodes one key press. `application_cursor` is the terminal's cursor key
/// mode (DECCKM), which full-screen programs such as `vim` turn on.
#[must_use]
pub fn encode_key(key: Key, modifiers: Modifiers, application_cursor: bool) -> Vec<u8> {
    let mut out = Vec::new();
    match key {
        Key::Char(character) => {
            if modifiers.alt {
                out.push(0x1b);
            }
            match (modifiers.ctrl, control(character)) {
                (true, Some(byte)) => out.push(byte),
                _ => {
                    let mut buffer = [0; 4];
                    out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                }
            }
        }
        Key::Enter => simple(&mut out, modifiers, b"\r"),
        Key::Tab if modifiers.shift => out.extend_from_slice(b"\x1b[Z"),
        Key::Tab => simple(&mut out, modifiers, b"\t"),
        Key::BackTab => out.extend_from_slice(b"\x1b[Z"),
        Key::Backspace if modifiers.ctrl => simple(&mut out, Modifiers::NONE, b"\x08"),
        Key::Backspace => simple(&mut out, modifiers, b"\x7f"),
        Key::Escape => simple(&mut out, modifiers, b"\x1b"),
        Key::Up => cursor(&mut out, b'A', modifiers, application_cursor),
        Key::Down => cursor(&mut out, b'B', modifiers, application_cursor),
        Key::Right => cursor(&mut out, b'C', modifiers, application_cursor),
        Key::Left => cursor(&mut out, b'D', modifiers, application_cursor),
        Key::Home => cursor(&mut out, b'H', modifiers, application_cursor),
        Key::End => cursor(&mut out, b'F', modifiers, application_cursor),
        Key::Insert => tilde(&mut out, 2, modifiers),
        Key::Delete => tilde(&mut out, 3, modifiers),
        Key::PageUp => tilde(&mut out, 5, modifiers),
        Key::PageDown => tilde(&mut out, 6, modifiers),
        Key::F(number @ 1..=4) => {
            let last = b'P' + number - 1;
            if modifiers.any() {
                out.extend_from_slice(format!("\x1b[1;{}", modifiers.parameter()).as_bytes());
                out.push(last);
            } else {
                out.extend_from_slice(&[0x1b, b'O', last]);
            }
        }
        Key::F(number @ 5..=12) => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][usize::from(number - 5)];
            tilde(&mut out, code, modifiers);
        }
        Key::F(_) => {}
    }
    out
}

/// A key whose only modifier encoding is an escape prefix for Alt.
fn simple(out: &mut Vec<u8>, modifiers: Modifiers, bytes: &[u8]) {
    if modifiers.alt {
        out.push(0x1b);
    }
    out.extend_from_slice(bytes);
}

fn cursor(out: &mut Vec<u8>, last: u8, modifiers: Modifiers, application: bool) {
    if modifiers.any() {
        out.extend_from_slice(format!("\x1b[1;{}", modifiers.parameter()).as_bytes());
        out.push(last);
    } else if application {
        out.extend_from_slice(&[0x1b, b'O', last]);
    } else {
        out.extend_from_slice(&[0x1b, b'[', last]);
    }
}

fn tilde(out: &mut Vec<u8>, code: u8, modifiers: Modifiers) {
    if modifiers.any() {
        out.extend_from_slice(format!("\x1b[{code};{}~", modifiers.parameter()).as_bytes());
    } else {
        out.extend_from_slice(format!("\x1b[{code}~").as_bytes());
    }
}

/// Encodes pasted text. Line endings become carriage returns, as a typed
/// Enter sends. Escape and other control characters except tab are removed,
/// so pasted text cannot end a bracketed paste early or smuggle a control
/// sequence. With `bracketed` (mode 2004), the text is wrapped in the paste
/// markers.
#[must_use]
pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 12);
    if bracketed {
        out.extend_from_slice(b"\x1b[200~");
    }
    let mut previous_cr = false;
    for character in text.chars() {
        match character {
            '\r' => out.push(b'\r'),
            '\n' if previous_cr => {}
            '\n' => out.push(b'\r'),
            '\t' => out.push(b'\t'),
            c if c.is_control() => {}
            c => {
                let mut buffer = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            }
        }
        previous_cr = character == '\r';
    }
    if bracketed {
        out.extend_from_slice(b"\x1b[201~");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_letters_and_punctuation() {
        assert_eq!(encode_key(Key::Char('c'), Modifiers::CTRL, false), b"\x03");
        assert_eq!(encode_key(Key::Char('C'), Modifiers::CTRL, false), b"\x03");
        assert_eq!(encode_key(Key::Char('['), Modifiers::CTRL, false), b"\x1b");
        assert_eq!(encode_key(Key::Char(' '), Modifiers::CTRL, false), b"\x00");
        assert_eq!(encode_key(Key::Char('?'), Modifiers::CTRL, false), b"\x7f");
        // No control form: the character itself.
        assert_eq!(encode_key(Key::Char('1'), Modifiers::CTRL, false), b"1");
    }

    #[test]
    fn plain_and_alt_characters() {
        assert_eq!(
            encode_key(Key::Char('é'), Modifiers::NONE, false),
            "é".as_bytes()
        );
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode_key(Key::Char('x'), alt, false), b"\x1bx");
        assert_eq!(encode_key(Key::Backspace, alt, false), b"\x1b\x7f");
    }

    #[test]
    fn cursor_keys_follow_the_cursor_key_mode() {
        assert_eq!(encode_key(Key::Up, Modifiers::NONE, false), b"\x1b[A");
        assert_eq!(encode_key(Key::Up, Modifiers::NONE, true), b"\x1bOA");
        assert_eq!(encode_key(Key::Left, Modifiers::CTRL, true), b"\x1b[1;5D");
        assert_eq!(encode_key(Key::Home, Modifiers::NONE, false), b"\x1b[H");
    }

    #[test]
    fn editing_and_function_keys() {
        assert_eq!(encode_key(Key::Enter, Modifiers::NONE, false), b"\r");
        assert_eq!(encode_key(Key::Tab, Modifiers::NONE, false), b"\t");
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode_key(Key::Tab, shift, false), b"\x1b[Z");
        assert_eq!(encode_key(Key::Backspace, Modifiers::NONE, false), b"\x7f");
        assert_eq!(encode_key(Key::Backspace, Modifiers::CTRL, false), b"\x08");
        assert_eq!(encode_key(Key::Escape, Modifiers::NONE, false), b"\x1b");
        assert_eq!(encode_key(Key::Delete, Modifiers::NONE, false), b"\x1b[3~");
        assert_eq!(encode_key(Key::PageDown, shift, false), b"\x1b[6;2~");
        assert_eq!(encode_key(Key::F(1), Modifiers::NONE, false), b"\x1bOP");
        assert_eq!(encode_key(Key::F(5), Modifiers::NONE, false), b"\x1b[15~");
        assert_eq!(encode_key(Key::F(12), Modifiers::NONE, false), b"\x1b[24~");
        assert!(encode_key(Key::F(13), Modifiers::NONE, false).is_empty());
    }

    #[test]
    fn a_paste_normalizes_newlines_and_cannot_close_the_bracket() {
        assert_eq!(encode_paste("a\nb\r\nc", false), b"a\rb\rc");
        assert_eq!(
            encode_paste("x\x1b[201~rm -rf\n", true),
            b"\x1b[200~x[201~rm -rf\r\x1b[201~"
        );
        assert_eq!(encode_paste("\ttab", false), b"\ttab");
    }
}
