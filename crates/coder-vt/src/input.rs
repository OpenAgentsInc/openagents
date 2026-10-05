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
    /// A function key, F1 to F24. F13 to F24 send F1 to F12 with Shift,
    /// as xterm does.
    F(u8),
    /// A keypad key: a digit, `.`, `,`, `+`, `-`, `*`, `/`, or `=`. In the
    /// application keypad mode (DECKPAM) it sends `ESC O` and a letter.
    Keypad(char),
    /// The keypad's Enter.
    KeypadEnter,
}

/// The terminal modes that change what a key sends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModes {
    /// Cursor keys send `ESC O` (DECCKM, mode 1).
    pub application_cursor: bool,
    /// Keypad keys send `ESC O` (DECKPAM).
    pub application_keypad: bool,
    /// The Kitty keyboard flags the program negotiated ([`kitty`]); 0 keeps
    /// the xterm encoding.
    pub kitty: u8,
}

/// The Kitty keyboard protocol's progressive enhancement, as far as this
/// terminal supports it: the program asks with `CSI > flags u` and the
/// terminal keeps the flags it supports on a stack per screen.
///
/// Supported: flag 1, disambiguate escape codes. Escape, and any key with
/// Ctrl or Alt, sends `CSI code ; modifiers u`, as do Enter, Tab, and
/// Backspace with a modifier; text and the other keys send what they send
/// without the protocol. Not supported: 2 (press, repeat, and release
/// events), 4 (alternate keys), 8 (every key as an escape code), and 16
/// (associated text). A query reports only the flags in effect.
pub mod kitty {
    /// Disambiguate escape codes.
    pub const DISAMBIGUATE: u8 = 1;
    /// Every flag this terminal honors.
    pub const SUPPORTED: u8 = DISAMBIGUATE;
    /// The most entries a screen's stack holds; a push past it drops the
    /// oldest.
    pub const STACK: usize = 16;
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

    /// xterm's and Kitty's modifier parameter: 1 plus shift 1, alt 2, and
    /// control 4.
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
    encode_key_in(
        key,
        modifiers,
        KeyModes {
            application_cursor,
            application_keypad: false,
            kitty: 0,
        },
    )
}

/// Encodes one key press under the terminal's key `modes`.
#[must_use]
pub fn encode_key_in(key: Key, modifiers: Modifiers, modes: KeyModes) -> Vec<u8> {
    if modes.kitty & kitty::DISAMBIGUATE != 0
        && let Some(bytes) = disambiguated(key, modifiers)
    {
        return bytes;
    }
    let application_cursor = modes.application_cursor;
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
        Key::F(number @ 13..=24) => {
            let shifted = Modifiers {
                shift: true,
                ..modifiers
            };
            return encode_key_in(Key::F(number - 12), shifted, modes);
        }
        Key::F(_) => {}
        Key::Keypad(character) => match keypad_letter(character) {
            Some(letter) if modes.application_keypad && !modifiers.any() => {
                out.extend_from_slice(&[0x1b, b'O', letter]);
            }
            _ => return encode_key_in(Key::Char(character), modifiers, modes),
        },
        Key::KeypadEnter if modes.application_keypad && !modifiers.any() => {
            out.extend_from_slice(b"\x1bOM");
        }
        Key::KeypadEnter => simple(&mut out, modifiers, b"\r"),
    }
    out
}

/// A key as the Kitty protocol's disambiguate flag sends it, or `None` when
/// it sends what it sends without the protocol.
fn disambiguated(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
    let (code, modifiers) = match key {
        Key::Escape => (27, modifiers),
        Key::Enter if modifiers.any() => (13, modifiers),
        Key::Tab if modifiers.any() => (9, modifiers),
        Key::BackTab => (
            9,
            Modifiers {
                shift: true,
                ..modifiers
            },
        ),
        Key::Backspace if modifiers.any() => (127, modifiers),
        Key::Char(character) if modifiers.ctrl || modifiers.alt => {
            // The key's own code is its unshifted character.
            if character.is_ascii_uppercase() {
                (
                    u32::from(character.to_ascii_lowercase()),
                    Modifiers {
                        shift: true,
                        ..modifiers
                    },
                )
            } else {
                (u32::from(character), modifiers)
            }
        }
        _ => return None,
    };
    Some(if modifiers.any() {
        format!("\x1b[{code};{}u", modifiers.parameter()).into_bytes()
    } else {
        format!("\x1b[{code}u").into_bytes()
    })
}

/// The final letter a keypad key sends in the application keypad mode.
fn keypad_letter(character: char) -> Option<u8> {
    Some(match character {
        '0'..='9' => b'p' + (character as u8 - b'0'),
        '*' => b'j',
        '+' => b'k',
        ',' => b'l',
        '-' => b'm',
        '.' => b'n',
        '/' => b'o',
        '=' => b'X',
        _ => return None,
    })
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
        assert_eq!(encode_key(Key::F(13), Modifiers::NONE, false), b"\x1b[1;2P");
        assert_eq!(
            encode_key(Key::F(17), Modifiers::NONE, false),
            b"\x1b[15;2~"
        );
        assert_eq!(
            encode_key(Key::F(24), Modifiers::NONE, false),
            b"\x1b[24;2~"
        );
        assert!(encode_key(Key::F(25), Modifiers::NONE, false).is_empty());
    }

    #[test]
    fn modified_navigation_keys_carry_the_xterm_parameter() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            alt: false,
        };
        assert_eq!(encode_key(Key::Right, Modifiers::CTRL, false), b"\x1b[1;5C");
        assert_eq!(encode_key(Key::Left, alt, false), b"\x1b[1;3D");
        assert_eq!(encode_key(Key::End, ctrl_shift, true), b"\x1b[1;6F");
        assert_eq!(
            encode_key(Key::PageUp, Modifiers::CTRL, false),
            b"\x1b[5;5~"
        );
        assert_eq!(encode_key(Key::F(3), alt, false), b"\x1b[1;3R");
    }

    #[test]
    fn kitty_disambiguation_sends_csi_u_only_where_the_flag_asks() {
        let kitty = KeyModes {
            kitty: kitty::DISAMBIGUATE,
            ..KeyModes::default()
        };
        let none = Modifiers::NONE;
        let ctrl = Modifiers::CTRL;
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        let cases: [(Key, Modifiers, &[u8]); 14] = [
            (Key::Escape, none, b"\x1b[27u"),
            (Key::Escape, alt, b"\x1b[27;3u"),
            (Key::Char('i'), ctrl, b"\x1b[105;5u"),
            (Key::Char('I'), ctrl, b"\x1b[105;6u"),
            (Key::Char('a'), alt, b"\x1b[97;3u"),
            (Key::Char(' '), ctrl, b"\x1b[32;5u"),
            (Key::Enter, shift, b"\x1b[13;2u"),
            (Key::Tab, ctrl, b"\x1b[9;5u"),
            (Key::BackTab, none, b"\x1b[9;2u"),
            (Key::Backspace, alt, b"\x1b[127;3u"),
            // Text, and Enter alone, and other keys are unchanged.
            (Key::Char('A'), shift, b"A"),
            (Key::Char('\u{e9}'), none, "\u{e9}".as_bytes()),
            (Key::Enter, none, b"\r"),
            (Key::Up, ctrl, b"\x1b[1;5A"),
        ];
        for (key, modifiers, bytes) in cases {
            assert_eq!(
                encode_key_in(key, modifiers, kitty),
                bytes,
                "{key:?} {modifiers:?}"
            );
        }
        // Without the flag, the xterm bytes.
        assert_eq!(
            encode_key_in(Key::Escape, none, KeyModes::default()),
            b"\x1b"
        );
        assert_eq!(
            encode_key_in(Key::Char('i'), ctrl, KeyModes::default()),
            b"\t"
        );
    }

    #[test]
    fn keypad_keys_follow_the_keypad_mode() {
        let normal = KeyModes::default();
        let application = KeyModes {
            application_keypad: true,
            ..KeyModes::default()
        };
        let none = Modifiers::NONE;
        assert_eq!(encode_key_in(Key::Keypad('7'), none, normal), b"7");
        assert_eq!(
            encode_key_in(Key::Keypad('7'), none, application),
            b"\x1bOw"
        );
        assert_eq!(
            encode_key_in(Key::Keypad('0'), none, application),
            b"\x1bOp"
        );
        assert_eq!(
            encode_key_in(Key::Keypad('+'), none, application),
            b"\x1bOk"
        );
        assert_eq!(
            encode_key_in(Key::Keypad('.'), none, application),
            b"\x1bOn"
        );
        assert_eq!(encode_key_in(Key::KeypadEnter, none, normal), b"\r");
        assert_eq!(
            encode_key_in(Key::KeypadEnter, none, application),
            b"\x1bOM"
        );
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
