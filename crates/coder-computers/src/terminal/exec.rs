//! One command run on a linked host: `openagents computer exec`'s core.
//!
//! [`run`] opens the host's shell with NIP-HOST `terminal.open`, attaches
//! with NIP-TERM through a [`Session`], turns echo off, prints
//! [`START_MARK`], and `exec`s the command, so the shell's exit is the
//! command's exit and everything after the mark is the command's output.
//! The host checks the `terminal` right on every request. The call blocks
//! its thread until the command ends, [`Limits::timeout`] passes (the shell
//! is closed and the run reports exit 124), or the session ends.
//!
//! The terminal client (`openagents computer exec`, `watch`, `tail`) and
//! the phone's chat, running a read-only command an offer proposed, both
//! use it.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::runtime::Handle;

use super::session::{Links, Session};
use super::{Model, Phase};

/// Marks where the echoed command line ends and the command's output starts.
pub const START_MARK: u8 = 0x1e;

/// The exit a run reports when it timed out, as `timeout(1)` does.
pub const TIMED_OUT: i32 = 124;

/// How long a run may wait and last, and the size of its terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How long to wait for the host to open the shell.
    pub wait: Duration,
    /// How long the command may run before the shell is closed.
    pub timeout: Duration,
    pub rows: u16,
    pub cols: u16,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            wait: Duration::from_secs(15),
            timeout: Duration::from_secs(600),
            rows: 50,
            cols: 200,
        }
    }
}

/// One command's run on a host: what it wrote, how it ended, and the route.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub output: String,
    pub exit: i32,
    pub timed_out: bool,
    pub phase: Phase,
    pub route: Option<String>,
    pub seconds: f64,
}

impl Run {
    /// The run as JSON, as `openagents --json computer exec` prints it.
    #[must_use]
    pub fn json(&self, host: &str, words: &[String]) -> serde_json::Value {
        json!({
            "host": host, "command": words, "output": self.output, "exit": self.exit,
            "timed_out": self.timed_out, "shell": phase_json(&self.phase),
            "route": self.route, "seconds": self.seconds,
        })
    }
}

/// The remote shell's phase as JSON.
#[must_use]
pub fn phase_json(phase: &Phase) -> serde_json::Value {
    match phase {
        Phase::Exited {
            code,
            signal,
            cause,
        } => json!({ "phase": "exited", "code": code, "signal": signal, "cause": cause }),
        Phase::Refused(reason) => json!({ "phase": "refused", "reason": reason }),
        other => json!({ "phase": format!("{other:?}").to_lowercase() }),
    }
}

/// Quote `word` for a POSIX shell.
#[must_use]
pub fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./=:@%+,".contains(&b))
    {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The line typed into the shell: echo off, the mark, then `exec` of the
/// quoted words, so the shell's exit is the command's exit.
#[must_use]
pub fn line(words: &[String]) -> String {
    let command = words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ");
    format!("stty -echo 2>/dev/null; printf '\\036'; exec {command}\n")
}

/// Strips what precedes the mark from `chunk`: `None` until the mark is
/// seen, then the bytes after it. Once `started`, every chunk is output.
#[must_use]
pub fn after_mark(chunk: Vec<u8>, started: &mut bool) -> Option<Vec<u8>> {
    if *started {
        return Some(chunk);
    }
    let at = chunk.iter().position(|b| *b == START_MARK)?;
    *started = true;
    Some(chunk[at + 1..].to_vec())
}

/// The exit a finished phase reports; `failure` when the shell did not
/// exit on its own.
#[must_use]
pub fn exit_of(phase: &Phase, failure: i32) -> i32 {
    match phase {
        Phase::Exited { code, signal, .. } => code.or_else(|| signal.map(|s| 128 + s)).unwrap_or(1),
        _ => failure,
    }
}

/// A session on `host` with its output tapped.
#[must_use]
pub fn start(
    runtime: &Handle,
    links: Links,
    host: &str,
    rows: u16,
    cols: u16,
) -> (Session, mpsc::Receiver<Vec<u8>>) {
    let (tap, output) = mpsc::channel();
    let mut model = Model::new(host, host, rows, cols);
    model.tap = Some(tap);
    (Session::start(runtime, links, model), output)
}

/// Wait until the session is attached, or return the phase it ended in.
///
/// # Errors
///
/// The phase the session ended in, or a refusal when `deadline` passes.
pub fn attached(session: &Session, deadline: Instant) -> Result<(), Phase> {
    loop {
        let phase = session.model().phase.clone();
        match phase {
            Phase::Attached => return Ok(()),
            phase if phase.ended() => return Err(phase),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(Phase::Refused(
                "the host did not open a terminal in time; pass --wait SECONDS to wait longer"
                    .into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Run `words` on `host`, handing each output chunk to `sink` as it
/// arrives, and return the whole run once the command ends. Blocks this
/// thread; `runtime` drives the session.
///
/// # Errors
///
/// The shell was not opened: the phase the session ended in, described
/// (a refusal carries the host's reason, such as a missing right).
pub fn run(
    runtime: &Handle,
    links: Links,
    host: &str,
    words: &[String],
    limits: Limits,
    failure: i32,
    sink: &mut dyn FnMut(&[u8]),
) -> Result<Run, Phase> {
    let began = Instant::now();
    let (session, frames) = start(runtime, links, host, limits.rows, limits.cols);
    attached(&session, Instant::now() + limits.wait)?;
    session.send(line(words).into_bytes());

    let deadline = Instant::now() + limits.timeout;
    let mut bytes = Vec::new();
    let mut started = false;
    let mut timed_out = false;
    loop {
        match frames.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                if let Some(chunk) = after_mark(chunk, &mut started) {
                    sink(&chunk);
                    bytes.extend(chunk);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let phase = session.model().phase.clone();
        if phase.ended() {
            // Frames in flight arrive before the exit frame is applied.
            while let Ok(chunk) = frames.try_recv() {
                if let Some(chunk) = after_mark(chunk, &mut started) {
                    sink(&chunk);
                    bytes.extend(chunk);
                }
            }
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            session.close();
            break;
        }
    }
    let phase = session.model().phase.clone();
    let exit = exit_of(&phase, failure);
    let route = session.model().route.clone();
    Ok(Run {
        output: String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
        exit: if timed_out { TIMED_OUT } else { exit },
        timed_out,
        phase,
        route,
        seconds: began.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_a_posix_shell() {
        assert_eq!(quote("ls"), "ls");
        assert_eq!(quote("--model=gpt"), "--model=gpt");
        assert_eq!(quote("hello world"), "'hello world'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn the_line_marks_the_output_and_execs_the_command() {
        let words = ["openagents", "--json", "verse", "who"].map(String::from);
        assert_eq!(
            line(&words),
            "stty -echo 2>/dev/null; printf '\\036'; exec openagents --json verse who\n"
        );
    }

    #[test]
    fn output_starts_after_the_mark() {
        let mut started = false;
        assert_eq!(after_mark(b"$ stty -echo".to_vec(), &mut started), None);
        assert!(!started);
        assert_eq!(
            after_mark(b"echoed\x1e{\"a\"".to_vec(), &mut started),
            Some(b"{\"a\"".to_vec())
        );
        assert!(started);
        // After the mark, a later mark byte is output like any other.
        assert_eq!(
            after_mark(b":1}\x1e".to_vec(), &mut started),
            Some(b":1}\x1e".to_vec())
        );
    }

    #[test]
    fn a_phase_reports_its_exit() {
        let exited = |code, signal| Phase::Exited {
            code,
            signal,
            cause: "exited",
        };
        assert_eq!(exit_of(&exited(Some(0), None), 2), 0);
        assert_eq!(exit_of(&exited(None, Some(9)), 2), 137);
        assert_eq!(exit_of(&exited(None, None), 2), 1);
        assert_eq!(exit_of(&Phase::Lost, 2), 2);
        assert_eq!(
            phase_json(&Phase::Refused("no terminal right".into())),
            json!({ "phase": "refused", "reason": "no terminal right" })
        );
    }
}
