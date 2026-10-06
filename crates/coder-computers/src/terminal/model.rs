//! What a terminal screen shows: the session's phase and the emulated grid.

use coder_vt::{Key, Modifiers, Terminal};

/// The most rows a phone grid uses.
pub const MAX_ROWS: u16 = 80;
/// The most columns a phone grid uses.
pub const MAX_COLS: u16 = 240;
/// Lines of scrollback the emulator keeps.
pub(crate) const SCROLLBACK: usize = 500;

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

/// Who types at the terminal, as far as this screen knows (NIP-TERM's
/// typist feature).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    /// The host has not said, or does not serve the feature.
    Unknown,
    /// Nobody: the next device to type takes the role.
    Free,
    /// This screen.
    Mine,
    /// Another device. This screen draws at its size and cannot type until
    /// it takes the role.
    Elsewhere,
}

/// One command from the host's block journal, as the screen lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRow {
    /// The host's block number, increasing per terminal.
    pub number: u64,
    pub command: String,
    pub dir: String,
    /// `running`, `ok`, `exit N`, or `abandoned`.
    pub outcome: String,
}

/// What the screen knows of the terminal's block journal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Blocks {
    /// Not asked for, or hidden again.
    #[default]
    Hidden,
    /// Asked for; the host has not answered yet.
    Reading,
    /// A page, newest first, and whether older blocks remain.
    Page { rows: Vec<BlockRow>, more: bool },
    /// The host keeps no journal, or refused the read. The text says why.
    Unavailable(String),
}

/// One saved session as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedEntry {
    pub session: String,
    pub name: String,
    pub members: u16,
}

/// One member of a saved session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedMember {
    /// A host terminal. `state` is `live`, `closed`, `lost`, or `unknown`.
    Terminal {
        member: u16,
        generation: String,
        terminal: String,
        state: &'static str,
    },
    /// A linked chat thread, by its ID.
    Thread { member: u16, thread: String },
    /// Another workbench resource the phone shows as a reference only.
    Other {
        member: u16,
        kind: String,
        id: String,
    },
}

impl SavedMember {
    #[must_use]
    pub fn member(&self) -> u16 {
        match self {
            SavedMember::Terminal { member, .. }
            | SavedMember::Thread { member, .. }
            | SavedMember::Other { member, .. } => *member,
        }
    }
}

/// What the screen knows of the host's saved sessions (NIP-TERM's
/// sessions feature).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Saved {
    /// Not asked for, or hidden again.
    #[default]
    Hidden,
    /// Asked for; the host has not answered yet.
    Reading,
    /// The host's sessions.
    List(Vec<SavedEntry>),
    /// One session's members, in the order the host keeps them.
    Open {
        session: String,
        name: String,
        members: Vec<SavedMember>,
    },
    /// The host keeps no sessions, or refused. The text says why.
    Unavailable(String),
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
    /// Receives every output byte the host sends, before the emulator draws
    /// it, for a reader that wants the stream rather than the grid. After a
    /// join by snapshot, that is the output after the snapshot.
    pub tap: Option<std::sync::mpsc::Sender<Vec<u8>>>,
    /// Who types at the terminal.
    pub typing: Typing,
    /// This screen's own grid size. While another device types, the grid
    /// follows that device's size and the screen shows the part of it
    /// around the cursor.
    pub view: (u16, u16),
    /// A size to send the host now that this screen may set it.
    pub pending_resize: Option<(u16, u16)>,
    /// The terminal this screen is attached to, once the host named it, as
    /// `(generation, terminal)`. A screen recreated after the app returns
    /// from the background attaches to it again instead of opening another.
    pub reference: Option<(String, String)>,
    /// This screen only watches: it attaches in `observe` mode and sends
    /// no input. The host enforces the same.
    pub watch: bool,
    /// The block journal, when the person asked for it.
    pub blocks: Blocks,
    /// The host's saved sessions, when the person asked for them.
    pub saved: Saved,
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
            tap: None,
            typing: Typing::Unknown,
            view: (rows, cols),
            pending_resize: None,
            reference: None,
            watch: false,
            blocks: Blocks::Hidden,
            saved: Saved::Hidden,
        }
    }

    /// Record a change.
    pub fn touch(&mut self) {
        self.revision += 1;
    }

    /// Draw output the host sent, and pass it to the tap when one is set.
    pub fn output(&mut self, data: &[u8]) {
        if let Some(tap) = &self.tap
            && tap.send(data.to_vec()).is_err()
        {
            self.tap = None;
        }
        self.vt.feed(data);
        self.touch();
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

    /// Change the screen's size. Returns the clamped size when the grid
    /// changed with it; while another device types, the grid keeps that
    /// device's size.
    pub fn resize(&mut self, rows: u16, cols: u16) -> Option<(u16, u16)> {
        let (rows, cols) = clamp(rows, cols);
        if self.view != (rows, cols) {
            self.view = (rows, cols);
            self.touch();
        }
        if self.typing == Typing::Elsewhere {
            return None;
        }
        if usize::from(rows) == self.vt.rows() && usize::from(cols) == self.vt.cols() {
            return None;
        }
        self.vt.resize(usize::from(rows), usize::from(cols));
        self.touch();
        Some((rows, cols))
    }

    /// Records who types now. Another device's typing sets the grid to
    /// the terminal's `size`; this screen's own, or nobody's, returns the
    /// grid to the screen's size and asks for it on the host.
    pub fn seat(&mut self, typing: Typing, size: (u16, u16)) {
        if self.typing != typing {
            self.typing = typing;
            self.touch();
        }
        // A watcher always draws at the terminal's size and never sets it.
        let follows = typing == Typing::Elsewhere || self.watch;
        let grid = if follows { size } else { self.view };
        if (usize::from(grid.0), usize::from(grid.1)) != (self.vt.rows(), self.vt.cols()) {
            self.vt.resize(usize::from(grid.0), usize::from(grid.1));
            self.touch();
        }
        if !follows && size != self.view {
            self.pending_resize = Some(self.view);
        }
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
    fn a_viewer_draws_at_the_typists_size_and_returns_to_its_own() {
        let mut model = Model::new("h", "Mac", 10, 40);
        model.seat(Typing::Elsewhere, (30, 100));
        assert_eq!(model.size(), (30, 100));
        // The screen's own size changes, but the grid keeps the typist's.
        assert_eq!(model.resize(12, 44), None);
        assert_eq!(model.view, (12, 44));
        assert_eq!(model.size(), (30, 100));
        assert_eq!(model.pending_resize, None);
        // Taking the role returns the grid to the screen and asks the host.
        model.seat(Typing::Mine, (30, 100));
        assert_eq!(model.size(), (12, 44));
        assert_eq!(model.pending_resize, Some((12, 44)));
        // Already at the screen's size, nothing is asked.
        model.pending_resize = None;
        model.seat(Typing::Free, (12, 44));
        assert_eq!(model.pending_resize, None);
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
