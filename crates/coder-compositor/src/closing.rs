//! The close chord: what Super+W does to a window whose session has work
//! in flight.
//!
//! The bind used to be `killactive`, so one press closed the focused
//! window with nothing asked, and a window running a live agent closed the
//! same as an idle one. The owner hit that with a session live, and
//! `os/bin/coder-close` put one question in front of the key on the
//! Hyprland session. This module is that question inside the compositor.
//!
//! Three rules shape it, and they are the shell key's rules.
//!
//! **It asks the window.** Nothing here reads an app-id, a title, or a
//! list of applications. The compositor knows the focused window's
//! process, [`descendants`] collects that process and everything under it,
//! and the client answers whether any of them has work in flight. The
//! descendants matter: the window is a terminal emulator and the session
//! is its child, so the process the compositor holds is the terminal's and
//! the process that publishes the record is Coder's.
//!
//! **The rule lives in the client.** What counts as work in flight is
//! decided once, by `coder activity` in `crates/coder/src/activity.rs`: a
//! turn streaming, or a delegation that has not reported. This module reads the sentence that
//! verb prints and shows it. A second rule here would be a rule to
//! reconcile.
//!
//! **A key that cannot ask still closes.** A window with no process, a
//! client that is not installed, and a client that fails are each a close.
//! The alternative is a window a person cannot shut, and the question
//! exists to save an agent's work rather than to hold a desktop hostage.
//!
//! The confirmation is a second press rather than a dialog with buttons. A
//! dialog that waits on an answer holds the key open, and a desktop whose
//! notification daemon is not running would leave a window that never
//! closes. A second press degrades the other way: at worst the notice is
//! unseen and the next press closes the window, which is the behavior this
//! replaced. The notice is the compositor's own, so `mako` is not required
//! for it.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use coder_wm::WinId;

use crate::state::Coder;

/// How long the press after a notice counts. Past it the key asks again,
/// so a notice nobody saw does not arm the key for the rest of the
/// session.
pub const WINDOW: Duration = Duration::from_secs(5);

/// How long a notice stays on the screen.
pub const NOTICE: Duration = Duration::from_secs(5);

/// The client that answers the question. A bare name resolves on the
/// session's `PATH`, which is where `coderos.desktop.command` finds the
/// same binary.
pub const CLIENT_VAR: &str = "CODER_CLOSE_CLIENT";

/// What the chord does with one window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Close it now.
    Close,
    /// Say this, and close on the press after.
    Ask(String),
}

/// Whether anything under one window has work in flight, and the sentence
/// that says what. A test answers from a fixture; the session answers from
/// `coder activity --pid`.
pub trait Activity {
    /// The sentence for the processes `pids` names, or nothing when none
    /// of them is working and when the question could not be asked.
    fn busy(&self, pids: &[i64]) -> Option<String>;
}

/// The question, asked of the client on this session's `PATH`.
pub struct Client {
    /// The binary that answers, `coder` on a host.
    pub program: String,
}

impl Client {
    /// The client this session names, or `coder`.
    pub fn read() -> Client {
        Client {
            program: match std::env::var(CLIENT_VAR) {
                Ok(named) if !named.trim().is_empty() => named,
                _ => "coder".to_string(),
            },
        }
    }
}

impl Activity for Client {
    fn busy(&self, pids: &[i64]) -> Option<String> {
        if pids.is_empty() {
            return None;
        }
        let mut command = Command::new(&self.program);
        command.arg("activity");
        for pid in pids {
            command.arg("--pid").arg(pid.to_string());
        }
        // The answer comes back before the frame after this one: the verb
        // reads a file a session wrote and exits. A client that is not
        // installed and a client that fails are both an error here, and
        // both close the window.
        let answered = command.stdin(Stdio::null()).stderr(Stdio::null()).output();
        let out = answered.ok()?;
        if !out.status.success() {
            return None;
        }
        let sentence = String::from_utf8_lossy(&out.stdout).trim().to_string();
        match sentence.is_empty() {
            true => None,
            false => Some(sentence),
        }
    }
}

/// What the compositor remembers between two presses: the window a notice
/// was raised for, and the moment the press after it stops counting.
#[derive(Default)]
pub struct Closing {
    armed: Option<(WinId, Instant)>,
}

impl Closing {
    /// What the press on `id` does, given what the client answered.
    ///
    /// An idle window, a window the client knows nothing about, and a
    /// client that could not answer are one answer here, and each closes
    /// at once. A window with work in flight takes a notice and the press
    /// after it, on the same window and inside [`WINDOW`].
    pub fn decide(&mut self, id: WinId, busy: Option<String>, now: Instant) -> Decision {
        let Some(sentence) = busy else {
            self.armed = None;
            return Decision::Close;
        };
        if let Some((held, until)) = self.armed
            && held == id
            && now <= until
        {
            self.armed = None;
            return Decision::Close;
        }
        self.armed = Some((id, now + WINDOW));
        Decision::Ask(sentence)
    }
}

/// A notice the operator reads: what the compositor says, and when it
/// stops saying it.
///
/// The desk protocol's `notice` verb and the close chord raise them
/// through one holder, so a session that runs this compositor needs no
/// notification daemon for either.
#[derive(Default)]
pub struct Notices {
    /// The text of the notice showing, and the moment it goes away.
    showing: Option<(String, Instant)>,
}

impl Notices {
    /// Raises one notice, replacing whatever was showing.
    pub fn raise(&mut self, text: &str, now: Instant) {
        log::info!("notice: {text}");
        self.showing = Some((text.to_string(), now + NOTICE));
    }

    /// The notice showing at `now`, or nothing.
    pub fn showing(&self, now: Instant) -> Option<&str> {
        self.showing
            .as_ref()
            .filter(|(_, until)| *until > now)
            .map(|(text, _)| text.as_str())
    }
}

/// `root` and every process under it, from a table of `(process, parent)`
/// pairs. This is the whole of what the client is asked about, so a
/// session in another window is never read for this one.
pub fn descendants(table: &[(i64, i64)], root: i64) -> Vec<i64> {
    let mut want = vec![root];
    loop {
        let mut added = false;
        for (pid, parent) in table {
            if !want.contains(pid) && want.contains(parent) {
                want.push(*pid);
                added = true;
            }
        }
        if !added {
            return want;
        }
    }
}

/// This machine's process table, as `(process, parent)` pairs.
///
/// It is read from `/proc` rather than from `ps`, because the compositor
/// runs on Linux alone and a key press should not wait on a process to
/// start. A host that answers nothing here leaves the window itself as the
/// one process the client is asked about, which is the answer for a
/// session the compositor started directly.
pub fn process_table() -> Vec<(i64, i64)> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut table = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|name| name.parse::<i64>().ok()) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(parent) = parent_of(&stat) {
            table.push((pid, parent));
        }
    }
    table
}

/// The parent one `/proc/<pid>/stat` line names. The command name sits in
/// parentheses and can hold a space or a parenthesis of its own, so the
/// fields are counted from the last `)` rather than from the start.
fn parent_of(stat: &str) -> Option<i64> {
    let (_, after) = stat.rsplit_once(')')?;
    after.split_whitespace().nth(1)?.parse().ok()
}

impl Coder {
    /// Raises one notice the operator reads.
    pub fn raise_notice(&mut self, text: &str) {
        self.notices.raise(text, Instant::now());
    }

    /// What the close chord does to one window: it asks the client about
    /// every process under the window, and closes on the answer.
    pub fn close_chord(&mut self, id: WinId, activity: &dyn Activity) {
        let pid = self.tile(id).and_then(|tile| tile.pid);
        let asked = match pid.filter(|pid| *pid > 0) {
            Some(pid) => descendants(&process_table(), pid),
            None => Vec::new(),
        };
        let busy = match asked.is_empty() {
            true => None,
            false => activity.busy(&asked),
        };
        match self.closing.decide(id, busy, Instant::now()) {
            Decision::Close => self.close_window(id),
            Decision::Ask(sentence) => {
                let said = format!(
                    "{sentence} Press the close chord again within {}s to close this window.",
                    WINDOW.as_secs()
                );
                self.raise_notice(&said);
            }
        }
    }
}

#[cfg(test)]
#[path = "closing_tests.rs"]
mod tests;
