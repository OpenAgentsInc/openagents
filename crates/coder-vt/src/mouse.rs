//! Mouse reports a program asks for, as xterm encodes them.
//!
//! A program turns reporting on with a tracking mode (9, 1000, 1002, or
//! 1003) and picks an encoding (the default bytes, UTF-8 with 1005, SGR
//! with 1006, or urxvt with 1015). [`encode_mouse`] turns one event into
//! the report, or nothing when the mode does not ask for that event.

use crate::input::Modifiers;

/// Which mouse events the program hears about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MouseMode {
    /// None: the client keeps the mouse (selection, scrolling).
    #[default]
    Off,
    /// Presses only, without modifiers (mode 9, X10).
    Press,
    /// Presses, releases, and the wheel (mode 1000).
    Click,
    /// As [`MouseMode::Click`], and motion while a button is held (1002).
    Drag,
    /// As [`MouseMode::Click`], and all motion (1003).
    Motion,
}

/// How a report is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MouseEncoding {
    /// `CSI M` and three bytes; positions past 223 cannot be sent.
    #[default]
    Default,
    /// `CSI M` with positions as UTF-8 characters (mode 1005).
    Utf8,
    /// `CSI < b ; x ; y M` or `m` for a release (mode 1006).
    Sgr,
    /// `CSI b ; x ; y M` (mode 1015).
    Urxvt,
}

/// A mouse button or wheel direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

impl MouseButton {
    fn code(self) -> u32 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp => 64,
            MouseButton::WheelDown => 65,
            MouseButton::WheelLeft => 66,
            MouseButton::WheelRight => 67,
        }
    }

    fn wheel(self) -> bool {
        self.code() >= 64
    }
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseKind {
    Press(MouseButton),
    Release(MouseButton),
    /// The pointer moved to a new cell, with the button held, if any.
    Motion(Option<MouseButton>),
}

/// One mouse event at a cell, zero-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseKind,
    pub row: usize,
    pub col: usize,
    pub modifiers: Modifiers,
}

/// The report `event` sends under `mode` and `encoding`, or `None` when
/// the program did not ask for it.
#[must_use]
pub fn encode_mouse(
    event: MouseEvent,
    mode: MouseMode,
    encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    let (code, release) = match (mode, event.kind) {
        (MouseMode::Off, _) => return None,
        (MouseMode::Press, MouseKind::Press(button)) if !button.wheel() => (button.code(), false),
        (MouseMode::Press, _) => return None,
        (_, MouseKind::Press(button)) => (button.code(), false),
        // A wheel has no release.
        (_, MouseKind::Release(button)) if button.wheel() => return None,
        (_, MouseKind::Release(button)) => (button.code(), true),
        (MouseMode::Drag, MouseKind::Motion(Some(button))) => (button.code() + 32, false),
        (MouseMode::Motion, MouseKind::Motion(held)) => {
            (held.map_or(3, MouseButton::code) + 32, false)
        }
        (_, MouseKind::Motion(_)) => return None,
    };
    let modifiers = if mode == MouseMode::Press {
        0
    } else {
        let m = event.modifiers;
        4 * u32::from(m.shift) + 8 * u32::from(m.alt) + 16 * u32::from(m.ctrl)
    };
    // Every encoding but SGR reports a release as button 3.
    let legacy = if release { 3 } else { code } + modifiers;
    let (x, y) = (event.col + 1, event.row + 1);
    match encoding {
        MouseEncoding::Sgr => Some(
            format!(
                "\x1b[<{};{x};{y}{}",
                code + modifiers,
                if release { 'm' } else { 'M' }
            )
            .into_bytes(),
        ),
        MouseEncoding::Urxvt => Some(format!("\x1b[{};{x};{y}M", legacy + 32).into_bytes()),
        MouseEncoding::Utf8 => {
            let mut out = b"\x1b[M".to_vec();
            for value in [legacy + 32, x as u32 + 32, y as u32 + 32] {
                // UTF-8 reports stop at two-byte characters.
                let c = char::from_u32(value).filter(|_| value < 2048)?;
                let mut buffer = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            }
            Some(out)
        }
        MouseEncoding::Default => {
            let byte = |value: usize| u8::try_from(value + 32).ok();
            Some(vec![
                0x1b,
                b'[',
                b'M',
                u8::try_from(legacy + 32).ok()?,
                byte(x)?,
                byte(y)?,
            ])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: MouseKind, row: usize, col: usize) -> MouseEvent {
        MouseEvent {
            kind,
            row,
            col,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn nothing_is_reported_while_reporting_is_off() {
        let press = event(MouseKind::Press(MouseButton::Left), 0, 0);
        assert_eq!(
            encode_mouse(press, MouseMode::Off, MouseEncoding::Sgr),
            None
        );
    }

    #[test]
    fn default_encoding_offsets_by_32_and_reports_release_as_3() {
        let press = event(MouseKind::Press(MouseButton::Left), 4, 9);
        assert_eq!(
            encode_mouse(press, MouseMode::Click, MouseEncoding::Default).unwrap(),
            b"\x1b[M\x20\x2a\x25"
        );
        let release = event(MouseKind::Release(MouseButton::Left), 4, 9);
        assert_eq!(
            encode_mouse(release, MouseMode::Click, MouseEncoding::Default).unwrap(),
            b"\x1b[M\x23\x2a\x25"
        );
        // Past column 223 the default encoding has no byte.
        let far = event(MouseKind::Press(MouseButton::Left), 0, 230);
        assert_eq!(
            encode_mouse(far, MouseMode::Click, MouseEncoding::Default),
            None
        );
        // UTF-8 reaches it with a two-byte character.
        let utf8 = encode_mouse(far, MouseMode::Click, MouseEncoding::Utf8).unwrap();
        assert_eq!(&utf8[..4], b"\x1b[M\x20");
        assert_eq!(std::str::from_utf8(&utf8[4..]).unwrap(), "\u{107}!");
    }

    #[test]
    fn sgr_keeps_the_button_on_release_and_adds_modifiers() {
        let mut press = event(MouseKind::Press(MouseButton::Right), 0, 299);
        press.modifiers.ctrl = true;
        assert_eq!(
            encode_mouse(press, MouseMode::Click, MouseEncoding::Sgr).unwrap(),
            b"\x1b[<18;300;1M"
        );
        let release = event(MouseKind::Release(MouseButton::Right), 0, 299);
        assert_eq!(
            encode_mouse(release, MouseMode::Click, MouseEncoding::Sgr).unwrap(),
            b"\x1b[<2;300;1m"
        );
        let wheel = event(MouseKind::Press(MouseButton::WheelDown), 2, 3);
        assert_eq!(
            encode_mouse(wheel, MouseMode::Click, MouseEncoding::Sgr).unwrap(),
            b"\x1b[<65;4;3M"
        );
        assert_eq!(
            encode_mouse(
                event(MouseKind::Release(MouseButton::WheelDown), 2, 3),
                MouseMode::Click,
                MouseEncoding::Sgr
            ),
            None
        );
        assert_eq!(
            encode_mouse(press, MouseMode::Click, MouseEncoding::Urxvt).unwrap(),
            b"\x1b[50;300;1M"
        );
    }

    #[test]
    fn motion_follows_the_tracking_mode() {
        let drag = event(MouseKind::Motion(Some(MouseButton::Left)), 1, 1);
        let hover = event(MouseKind::Motion(None), 1, 1);
        assert_eq!(
            encode_mouse(drag, MouseMode::Click, MouseEncoding::Sgr),
            None
        );
        assert_eq!(
            encode_mouse(drag, MouseMode::Drag, MouseEncoding::Sgr).unwrap(),
            b"\x1b[<32;2;2M"
        );
        assert_eq!(
            encode_mouse(hover, MouseMode::Drag, MouseEncoding::Sgr),
            None
        );
        assert_eq!(
            encode_mouse(hover, MouseMode::Motion, MouseEncoding::Sgr).unwrap(),
            b"\x1b[<35;2;2M"
        );
        // X10 reports presses alone, without modifiers.
        let mut press = event(MouseKind::Press(MouseButton::Middle), 0, 0);
        press.modifiers.shift = true;
        assert_eq!(
            encode_mouse(press, MouseMode::Press, MouseEncoding::Default).unwrap(),
            b"\x1b[M\x21\x21\x21"
        );
        let release = event(MouseKind::Release(MouseButton::Middle), 0, 0);
        assert_eq!(
            encode_mouse(release, MouseMode::Press, MouseEncoding::Default),
            None
        );
    }
}
