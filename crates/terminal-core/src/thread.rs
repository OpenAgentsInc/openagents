//! The sheet's thread page: the conversation the input line's questions go
//! to, drawn natively in the transcript region (#10659).
//!
//! The page reads the thread through the same shared chat client the
//! questions go through (`openagents --json chat read --thread ID`, which
//! the mount's transport runs), so there is one client and one thread
//! store. The `openagents chat read` command reads the same thread ID in a
//! plain TTY. Opening, reopening, or refreshing the page only reads: it
//! never sends a message, creates a thread, or retries a send. A thread the
//! device does not keep is shown as missing, and one the client cannot read
//! now as unavailable, with the reason.

use serde::Deserialize;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// The most bytes of helper output the page reads.
pub const READ_MAX: usize = 1024 * 1024;

/// How often an open page reads a thread again while a reply arrives.
pub const BUSY_EVERY: Duration = Duration::from_secs(2);

/// One turn of a thread, as the shared client reports it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Turn {
    /// `user` or `assistant`.
    pub role: String,
    #[serde(default)]
    pub text: String,
    /// Observation stopped before the reply's terminal result arrived.
    #[serde(default)]
    pub stopped: bool,
    /// The send command that created the turn, when the client kept it.
    #[serde(default)]
    pub request: Option<String>,
}

/// A whole thread, as `openagents --json chat read` answers it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Thread {
    pub thread: String,
    #[serde(default)]
    pub title: String,
    /// A reply is streaming into the thread now.
    #[serde(default)]
    pub busy: bool,
    /// Why the last message has no reply, when it has none.
    #[serde(default)]
    pub failure: Option<String>,
    #[serde(default)]
    pub turns: Vec<Turn>,
    /// The Coder run the thread started, when it started one.
    #[serde(default)]
    pub coder: Option<Link>,
}

/// A thread's Coder run: its task ID and the host it runs on (`local` is
/// this computer).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Link {
    pub host: String,
    pub task: String,
}

impl Page {
    /// The run the shown thread started, when it started one.
    #[must_use]
    pub fn run(&self) -> Option<&Link> {
        match &self.shown {
            Some(Ok(thread)) => thread.coder.as_ref(),
            _ => None,
        }
    }
}

/// Why a thread could not be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The shared client keeps no such thread. Nothing is created.
    Missing,
    /// The client cannot read the thread now; the reason is plain text.
    Unavailable(String),
}

/// What reading a thread answered.
pub type Read = Result<Thread, Unread>;

/// Decodes the shared client's answer to reading thread `asked`: one JSON
/// object, either the thread or `{"error": ...}`. An answer about another
/// thread is unavailable, never shown in its place.
#[must_use]
pub fn decode(output: &[u8], asked: &str) -> Read {
    if output.len() > READ_MAX {
        return Err(Unread::Unavailable(
            "the thread is too large to show".into(),
        ));
    }
    let text = String::from_utf8_lossy(output);
    let Some(line) = text.lines().rev().find(|line| !line.trim().is_empty()) else {
        return Err(Unread::Unavailable(
            "the chat client answered nothing".into(),
        ));
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Err(Unread::Unavailable(
            "the chat client's answer was not readable".into(),
        ));
    };
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        if error.contains("Thread not found") {
            return Err(Unread::Missing);
        }
        return Err(Unread::Unavailable(crate::ascii::ascii(error)));
    }
    let Ok(thread) = serde_json::from_value::<Thread>(value) else {
        return Err(Unread::Unavailable(
            "the chat client's answer was not a thread".into(),
        ));
    };
    if thread.thread != asked {
        return Err(Unread::Unavailable(
            "the chat client answered for another thread".into(),
        ));
    }
    Ok(thread)
}

/// The page's state: which thread it shows, the last read, and the read
/// in flight.
#[derive(Default)]
pub struct Page {
    /// The page is drawn in place of the transcript.
    pub open: bool,
    /// The thread the page shows; it stays across closing and reopening.
    pub thread: Option<String>,
    /// The last read that finished, kept while the next one runs.
    pub shown: Option<Read>,
    /// Lines scrolled back from the bottom.
    pub scroll: usize,
    /// A read in flight.
    pub reading: Option<Receiver<Read>>,
    /// The thread changed since the last read began.
    pub dirty: bool,
    /// When the last read began.
    pub read_at: Option<Instant>,
    /// Reads begun, for tests and the receipt.
    pub reads: u64,
}

impl Page {
    /// Shows thread `id`: the same thread keeps what it showed and where
    /// it was scrolled; another starts empty.
    pub fn show(&mut self, id: &str) {
        if self.thread.as_deref() != Some(id) {
            self.thread = Some(id.to_owned());
            self.shown = None;
            self.scroll = 0;
            self.reading = None;
        }
        self.open = true;
        self.dirty = true;
    }

    /// Whether the open page should read its thread again now.
    #[must_use]
    pub fn due(&self, now: Instant) -> bool {
        if !self.open || self.thread.is_none() || self.reading.is_some() {
            return false;
        }
        let busy = matches!(&self.shown, Some(Ok(thread)) if thread.busy);
        self.dirty
            || (busy
                && self
                    .read_at
                    .is_none_or(|at| now.duration_since(at) >= BUSY_EVERY))
    }

    /// The page's state word for its header.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match (&self.shown, self.reading.is_some()) {
            (None, _) => "reading",
            (Some(Ok(thread)), _) if thread.busy => "reply arriving",
            (Some(_), true) => "reading again",
            (Some(Ok(_)), false) => "current",
            (Some(Err(Unread::Missing)), false) => "missing",
            (Some(Err(Unread::Unavailable(_))), false) => "unavailable",
        }
    }
}

/// The page's text before wrapping: the header, then each turn, plainly.
#[must_use]
pub fn lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::{ascii, plain};
    use crate::paper::Tone;
    let mut out = Vec::new();
    let id = page.thread.as_deref().unwrap_or("-");
    let title = match &page.shown {
        Some(Ok(thread)) if !thread.title.trim().is_empty() => ascii(&thread.title),
        _ => "Untitled thread".into(),
    };
    let turns = match &page.shown {
        Some(Ok(thread)) => format!("  {} turns", thread.turns.len()),
        _ => String::new(),
    };
    out.push((
        format!("THREAD {title}{turns}  [{}]", page.state()),
        Tone::Loud,
    ));
    out.push((
        format!("TTY: openagents chat read --thread {}", ascii(id)),
        Tone::Quiet,
    ));
    if let Some(link) = page.run() {
        out.push((
            format!(
                "RUN {} on {}: F9 shows it",
                ascii(&link.task),
                ascii(&link.host)
            ),
            Tone::Present,
        ));
    }
    out.push((String::new(), Tone::Quiet));
    match &page.shown {
        None => out.push(("Reading the thread...".into(), Tone::Quiet)),
        Some(Err(Unread::Missing)) => out.push((
            "This thread is not on this computer. Nothing was created in its place.".into(),
            Tone::Present,
        )),
        Some(Err(Unread::Unavailable(why))) => out.push((
            format!("The thread can't be read now: {why}. F4 twice reads it again."),
            Tone::Present,
        )),
        Some(Ok(thread)) => {
            for turn in &thread.turns {
                let user = turn.role == "user";
                // A typed plan is data for a proposal, never page text.
                let text: String = turn
                    .text
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("{\"v\""))
                    .collect::<Vec<_>>()
                    .join("\n");
                let text = if user { ascii(&text) } else { plain(&text) };
                let (lead, tone) = if user {
                    ("YOU: ", Tone::Loud)
                } else {
                    ("OPENAGENTS: ", Tone::Present)
                };
                for (index, text) in text.lines().enumerate() {
                    let lead = if index == 0 {
                        lead.to_owned()
                    } else {
                        " ".repeat(lead.len())
                    };
                    out.push((format!("{lead}{text}"), tone));
                }
                if turn.stopped {
                    out.push(("[stopped before the reply finished]".into(), Tone::Quiet));
                }
                out.push((String::new(), Tone::Quiet));
            }
            if thread.turns.is_empty() {
                out.push(("No messages yet.".into(), Tone::Quiet));
            }
            if thread.busy {
                out.push(("[a reply is arriving]".into(), Tone::Quiet));
            }
            if let Some(failure) = &thread.failure {
                out.push((format!("NOTE: {}", ascii(failure)), Tone::Quiet));
            }
        }
    }
    out
}
