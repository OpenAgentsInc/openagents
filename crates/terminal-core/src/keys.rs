//! What a key sends to a pane's program: named keys through `coder-vt`'s
//! xterm encoder (function keys to F24, the keypad under its mode, and
//! modifier parameters for Ctrl, Alt, and Shift), Ctrl combinations as
//! control characters, and Option as Meta on macOS.

use crate::input::{KeyCode, Logical, NamedKey};
use coder_vt::{Key, Modifiers, Terminal};

use super::KeyIn;

/// The keypad key a physical key is, if any.
fn keypad(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Numpad0 => Key::Keypad('0'),
        KeyCode::Numpad1 => Key::Keypad('1'),
        KeyCode::Numpad2 => Key::Keypad('2'),
        KeyCode::Numpad3 => Key::Keypad('3'),
        KeyCode::Numpad4 => Key::Keypad('4'),
        KeyCode::Numpad5 => Key::Keypad('5'),
        KeyCode::Numpad6 => Key::Keypad('6'),
        KeyCode::Numpad7 => Key::Keypad('7'),
        KeyCode::Numpad8 => Key::Keypad('8'),
        KeyCode::Numpad9 => Key::Keypad('9'),
        KeyCode::NumpadDecimal => Key::Keypad('.'),
        KeyCode::NumpadComma => Key::Keypad(','),
        KeyCode::NumpadAdd => Key::Keypad('+'),
        KeyCode::NumpadSubtract => Key::Keypad('-'),
        KeyCode::NumpadMultiply => Key::Keypad('*'),
        KeyCode::NumpadDivide => Key::Keypad('/'),
        KeyCode::NumpadEqual => Key::Keypad('='),
        KeyCode::NumpadEnter => Key::KeypadEnter,
        _ => return None,
    })
}

/// The named key `named` is, if the encoder has one.
fn named(named: &NamedKey, shift: bool) -> Option<Key> {
    Some(match named {
        NamedKey::Enter => Key::Enter,
        NamedKey::Tab if shift => Key::BackTab,
        NamedKey::Tab => Key::Tab,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Escape => Key::Escape,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::Insert => Key::Insert,
        NamedKey::Delete => Key::Delete,
        NamedKey::Space => Key::Char(' '),
        NamedKey::F1 => Key::F(1),
        NamedKey::F2 => Key::F(2),
        NamedKey::F3 => Key::F(3),
        NamedKey::F4 => Key::F(4),
        NamedKey::F5 => Key::F(5),
        NamedKey::F6 => Key::F(6),
        NamedKey::F7 => Key::F(7),
        NamedKey::F8 => Key::F(8),
        NamedKey::F9 => Key::F(9),
        NamedKey::F10 => Key::F(10),
        NamedKey::F11 => Key::F(11),
        NamedKey::F12 => Key::F(12),
        NamedKey::F13 => Key::F(13),
        NamedKey::F14 => Key::F(14),
        NamedKey::F15 => Key::F(15),
        NamedKey::F16 => Key::F(16),
        NamedKey::F17 => Key::F(17),
        NamedKey::F18 => Key::F(18),
        NamedKey::F19 => Key::F(19),
        NamedKey::F20 => Key::F(20),
        NamedKey::F21 => Key::F(21),
        NamedKey::F22 => Key::F(22),
        NamedKey::F23 => Key::F(23),
        NamedKey::F24 => Key::F(24),
        _ => return None,
    })
}

/// The character a key types with no modifier but Shift: from the layout
/// when the platform says, else from the physical key on a US layout.
fn unmodified(key: &KeyIn, shift: bool) -> Option<char> {
    let base = key
        .plain
        .as_deref()
        .and_then(|plain| plain.chars().next())
        .or_else(|| us_layout(key.code))?;
    Some(if shift { shifted(base) } else { base })
}

fn shifted(c: char) -> char {
    if c.is_ascii_lowercase() {
        return c.to_ascii_uppercase();
    }
    match c {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        other => other,
    }
}

fn us_layout(code: KeyCode) -> Option<char> {
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let letter = |i: usize| letters.chars().nth(i);
    Some(match code {
        KeyCode::KeyA => letter(0)?,
        KeyCode::KeyB => letter(1)?,
        KeyCode::KeyC => letter(2)?,
        KeyCode::KeyD => letter(3)?,
        KeyCode::KeyE => letter(4)?,
        KeyCode::KeyF => letter(5)?,
        KeyCode::KeyG => letter(6)?,
        KeyCode::KeyH => letter(7)?,
        KeyCode::KeyI => letter(8)?,
        KeyCode::KeyJ => letter(9)?,
        KeyCode::KeyK => letter(10)?,
        KeyCode::KeyL => letter(11)?,
        KeyCode::KeyM => letter(12)?,
        KeyCode::KeyN => letter(13)?,
        KeyCode::KeyO => letter(14)?,
        KeyCode::KeyP => letter(15)?,
        KeyCode::KeyQ => letter(16)?,
        KeyCode::KeyR => letter(17)?,
        KeyCode::KeyS => letter(18)?,
        KeyCode::KeyT => letter(19)?,
        KeyCode::KeyU => letter(20)?,
        KeyCode::KeyV => letter(21)?,
        KeyCode::KeyW => letter(22)?,
        KeyCode::KeyX => letter(23)?,
        KeyCode::KeyY => letter(24)?,
        KeyCode::KeyZ => letter(25)?,
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        KeyCode::Backquote => '`',
        _ => return None,
    })
}

/// The bytes `key` sends to the program behind `vt` with `modifiers`
/// held. With `option_as_meta` on macOS, Option sends Escape before the
/// key's own character instead of the character macOS composes, as Meta
/// does elsewhere.
#[must_use]
pub fn encode(
    key: &KeyIn,
    modifiers: Modifiers,
    vt: &Terminal,
    option_as_meta: bool,
    macos: bool,
) -> Option<Vec<u8>> {
    if let Some(keypad) = keypad(key.code) {
        return Some(vt.key(keypad, modifiers));
    }
    let named = match &key.logical {
        Logical::Named(name) => named(name, modifiers.shift),
        _ => None,
    };
    if let Some(named) = named {
        // Shift alone is in the key itself for these.
        let modifiers = Modifiers {
            shift: modifiers.shift && !matches!(named, Key::BackTab | Key::Char(' ')),
            ..modifiers
        };
        return Some(vt.key(named, modifiers));
    }
    // Option composes characters on macOS; as Meta it sends the key's own
    // character after Escape.
    let meta = modifiers.alt && (!macos || option_as_meta);
    if modifiers.ctrl {
        let c = match &key.logical {
            Logical::Character(s) if !(macos && modifiers.alt) => s.chars().next(),
            _ => None,
        }
        .or_else(|| unmodified(key, false))?;
        // Under the Kitty protocol, Ctrl+Shift is its own chord; the
        // xterm encoding has no room for Shift there.
        let kitty = vt.kitty_flags() != 0;
        return Some(vt.key(
            Key::Char(if kitty { c.to_ascii_lowercase() } else { c }),
            Modifiers {
                shift: kitty && modifiers.shift,
                alt: meta,
                ctrl: true,
            },
        ));
    }
    if meta && macos {
        let c = unmodified(key, modifiers.shift)?;
        return Some(vt.key(
            Key::Char(c),
            Modifiers {
                alt: true,
                ..Modifiers::NONE
            },
        ));
    }
    let text = key
        .text
        .as_deref()
        .filter(|t| !t.is_empty())
        .or(match &key.logical {
            Logical::Character(s) => Some(s.as_str()),
            _ => None,
        })?;
    let mut bytes = Vec::new();
    if meta {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(text.as_bytes());
    Some(bytes)
}
