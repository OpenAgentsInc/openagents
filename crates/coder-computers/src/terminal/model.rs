//! What a terminal screen shows: the session's phase and the emulated grid.

use coder_vt::{Key, Modifiers, Terminal};

/// The most rows a phone grid uses.
pub const MAX_ROWS: u16 = 80;
/// The most columns a phone grid uses.
pub const MAX_COLS: u16 = 240;
/// Lines of scrollback the emulator keeps.
const SCROLLBACK: usize = 500;

/// Where the terminal session stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for the host's connection.
    Connecting,
    /// Asking the host to open a shell.
    Opening,
    /// Attached and interactive.
    Attached,
    /// The route to the host dropped; attaching again after the last frame
    /// this screen applied.
    Reconnecting,
    /// The shell's process ended.
    Exited {
        code: Option<i32>,
        signal: Option<i32>,
        /// How it ended, in words: `exited`, `closed`, `idle`, or `shutdown`.
        cause: &'static str,
    },
    /// The host restarted, and the terminal did not survive it.
    Lost,
    /// The host no longer has this terminal.
    Closed,
    /// The host refused. The text says why.
    Refused(String),
    /// This screen detached. The shell keeps running on the host.
    Left,
}

impl Phase {
    /// Whether the session is over and no more output will arrive.
    #[must_use]
    pub fn ended(&self) -> bool {
        matches!(
            self,
            Phase::Exited { .. } | Phase::Lost | Phase::Closed | Phase::Refused(_) | Phase::Left
        )
    }

    /// The status line for this phase.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Phase::Connecting => "Connecting to the computer…".into(),
            Phase::Opening => "Opening a shell…".into(),
            Phase::Attached => "Connected".into(),
            Phase::Reconnecting => "Reconnecting. Output you missed will follow.".into(),
            Phase::Exited {
                code,
                signal,
                cause,
            } => match (*cause, code, signal) {
                ("closed", ..) => "Terminal closed.".into(),
                ("idle", ..) => "The computer ended this idle terminal.".into(),
                ("shutdown", ..) => "The computer's host shut down. The terminal ended.".into(),
                (_, Some(code), _) => format!("The shell exited with code {code}."),
                (_, _, Some(signal)) => format!("The shell ended on signal {signal}."),
                _ => "The shell exited.".into(),
            },
            Phase::Lost => {
                "Lost: the computer restarted, and this terminal didn't survive it. Open a new one."
                    .into()
            }
            Phase::Closed => "This terminal no longer exists on the computer.".into(),
            Phase::Refused(reason) => reason.clone(),
            Phase::Left => "Detached. The shell keeps running on the computer.".into(),
        }
    }
}

/// The screen's state. The session task writes the phase and output; the
/// screen writes the modifier the accessory row latched.
#[derive(Debug)]
pub struct Model {
    /// The host's public key.
    pub host: String,
    /// The label the Computers screen shows for the host.
    pub label: String,
    pub phase: Phase,
    pub vt: Terminal,
    /// Output ranges the host discarded before this screen read them.
    pub gaps: u64,
    /// Bytes in those ranges, and whether some ranges had no count.
    pub missed: (u64, bool),
    /// The route in use, in words, while attached.
    pub route: Option<String>,
    /// Ctrl is latched for the next key.
    pub ctrl: bool,
    /// A refusal or failure to show once, such as input the host did not
    /// take.
    pub notice: Option<String>,
    /// Changes whenever anything here changes.
    pub revision: u64,
}

impl Model {
    /// A model for `host`, with a grid of `rows` by `cols`.
    #[must_use]
    pub fn new(host: impl Into<String>, label: impl Into<String>, rows: u16, cols: u16) -> Self {
        let (rows, cols) = clamp(rows, cols);
        Model {
            host: host.into(),
            label: label.into(),
            phase: Phase::Connecting,
            vt: Terminal::new(usize::from(rows), usize::from(cols), SCROLLBACK),
            gaps: 0,
            missed: (0, false),
            route: None,
            ctrl: false,
            notice: None,
            revision: 1,
        }
    }

    /// Record a change.
    pub fn touch(&mut self) {
        self.revision += 1;
    }

    pub fn set_phase(&mut self, phase: Phase) {
        if self.phase != phase {
            self.phase = phase;
            self.touch();
        }
    }

    /// Record a gap the host reported and mark it in the grid.
    pub fn gap(&mut self, bytes: Option<u64>) {
        self.gaps += 1;
        match bytes {
            Some(bytes) => self.missed.0 += bytes,
            None => self.missed.1 = true,
        }
        let text = match bytes {
            Some(bytes) => format!("[output lost: {} discarded by the host]", size(bytes)),
            None => "[output lost: the host discarded some output]".to_owned(),
        };
        self.vt.mark(&text);
        self.touch();
    }

    /// The bytes one key sends, applying a latched Ctrl once.
    pub fn key(&mut self, key: Key, mut modifiers: Modifiers) -> Vec<u8> {
        if self.ctrl {
            modifiers.ctrl = true;
            self.ctrl = false;
            self.touch();
        }
        self.vt.key(key, modifiers)
    }

    /// The bytes typed text sends. A latched Ctrl applies to the first
    /// character only; a line feed is Enter.
    pub fn text(&mut self, text: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for character in text.chars() {
            let key = match character {
                '\n' | '\r' => Key::Enter,
                '\t' => Key::Tab,
                c if c.is_control() => continue,
                c => Key::Char(c),
            };
            bytes.extend(self.key(key, Modifiers::NONE));
        }
        bytes
    }

    /// Change the grid size. Returns the clamped size when it changed.
    pub fn resize(&mut self, rows: u16, cols: u16) -> Option<(u16, u16)> {
        let (rows, cols) = clamp(rows, cols);
        if usize::from(rows) == self.vt.rows() && usize::from(cols) == self.vt.cols() {
            return None;
        }
        self.vt.resize(usize::from(rows), usize::from(cols));
        self.touch();
        Some((rows, cols))
    }

    /// The grid size.
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        // The emulator never exceeds the clamped size.
        (self.vt.rows() as u16, self.vt.cols() as u16)
    }
}

/// A grid size a phone uses: at least 2 by 10, at most [`MAX_ROWS`] by
/// [`MAX_COLS`].
#[must_use]
pub fn clamp(rows: u16, cols: u16) -> (u16, u16) {
    (rows.clamp(2, MAX_ROWS), cols.clamp(10, MAX_COLS))
}

/// A byte count in words.
fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} bytes"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gap_is_counted_and_marked_on_its_own_line() {
        let mut model = Model::new("h", "Mac", 4, 60);
        model.vt.feed(b"$ make");
        model.gap(Some(2048));
        model.gap(None);
        assert_eq!(model.gaps, 2);
        assert_eq!(model.missed, (2048, true));
        let text = model.vt.text();
        assert!(text.contains("$ make\n[output lost: 2.0 KB discarded by the host]"));
        assert!(text.contains("[output lost: the host discarded some output]"));
    }

    #[test]
    fn a_latched_ctrl_applies_to_one_key() {
        let mut model = Model::new("h", "Mac", 4, 40);
        model.ctrl = true;
        assert_eq!(model.text("cc"), b"\x03c");
        assert!(!model.ctrl);
        assert_eq!(model.text("ls\n"), b"ls\r");
        model.ctrl = true;
        assert_eq!(model.key(Key::Up, Modifiers::NONE), b"\x1b[1;5A");
    }

    #[test]
    fn sizes_are_clamped_and_changes_reported_once() {
        let mut model = Model::new("h", "Mac", 1, 1000);
        assert_eq!(model.size(), (2, MAX_COLS));
        assert_eq!(model.resize(30, 50), Some((30, 50)));
        assert_eq!(model.resize(30, 50), None);
    }

    #[test]
    fn phases_describe_themselves() {
        let exited = Phase::Exited {
            code: Some(2),
            signal: None,
            cause: "exited",
        };
        assert_eq!(exited.describe(), "The shell exited with code 2.");
        assert!(exited.ended());
        assert!(Phase::Lost.describe().starts_with("Lost"));
        assert!(!Phase::Reconnecting.ended());
    }
}
