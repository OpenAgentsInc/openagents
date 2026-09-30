//! The host's chat threads, read and continued by a device (NIP-HOST
//! `thread.list`, `thread.read`, and `thread.send`).
//!
//! A thread is one conversation with OpenAgents (`docs/glossary.md`,
//! Thread). On a computer, the host keeps its threads in its own encrypted
//! chat store; the desktop app and `openagents chat` reach them over the
//! host's local operator socket. These types carry them to a granted device
//! instead: a list of rows, one bounded page of a thread's turns with the
//! reply streaming into it, and a send that appends a follow-up through the
//! host. They are the wire form only; the host maps its chat service's
//! records to them, and a device maps them back to its own views.
//!
//! Nothing here carries the router's typed judgments, offers, or cards:
//! those stay on the host. A thread that delegated Coder work names the
//! task ([`ThreadCoder`]); the device reads that task through the observer
//! profile, as it reads any Coder chat.
use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};

/// The most rows a `threads` outcome carries: the host's newest threads
/// that are not archived.
pub const MAX_THREADS: usize = 128;
/// The most turns one `thread` page carries.
pub const MAX_TURNS: usize = 64;
/// The largest encoded `thread` or `threads` outcome, so a reply seals
/// well inside one relay frame (128 KiB, base64 and the envelope
/// included). A host drops older turns from a page, and shortens a single
/// turn's text, to stay inside it.
pub const MAX_PAGE_BYTES: usize = 48 * 1024;
/// The longest thread title.
pub const MAX_TITLE: usize = 160;
/// The longest message a device sends to a thread, as the chat service
/// allows.
pub const MAX_MESSAGE: usize = 32 * 1024;
/// The longest refusal or failure text a page carries.
pub const MAX_FAILURE: usize = 1024;

/// Who wrote a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadRole {
    User,
    Assistant,
}

/// Coder work a thread delegated: the task on the host that runs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadCoder {
    /// The Coder host's key, which may be this host.
    pub host: String,
    /// The Coder task's ID on that host.
    pub task: String,
    pub project: Option<String>,
    /// When the thread started it, in Unix seconds.
    pub at: Option<u64>,
}

/// One thread's row in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadRow {
    /// The thread ID: 32 lowercase hex characters.
    pub thread: String,
    pub title: String,
    pub started: u64,
    /// When its last message was sent or answered.
    pub updated: u64,
    pub pinned: bool,
    pub coder: Option<ThreadCoder>,
}

/// One turn of a thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadTurn {
    pub role: ThreadRole,
    pub text: String,
    /// When the host saved it; older turns have none.
    pub at: Option<u64>,
    /// The reply stopped before the worker's result arrived.
    pub stopped: bool,
    /// The model the worker named for a reply: an attribution claim.
    pub model: Option<String>,
    /// The send ID that created a message, whichever device sent it: 32
    /// lowercase hex characters.
    pub request: Option<String>,
}

/// One bounded page of a thread, newest turns last, and the reply
/// streaming into it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadPage {
    pub thread: String,
    pub title: String,
    /// The index of the first turn on this page; read before it for earlier
    /// turns.
    pub start: u64,
    /// How many turns the thread keeps.
    pub total: u64,
    pub turns: Vec<ThreadTurn>,
    /// A reply is streaming into the thread now.
    pub busy: bool,
    /// The streaming reply so far; empty when there is none.
    pub partial: String,
    /// Why the last message has no reply, when it has none.
    pub failure: Option<String>,
    pub coder: Option<ThreadCoder>,
}

/// Whether `id` is a thread or send ID: 32 lowercase hex characters, as the
/// chat service mints them.
#[must_use]
pub fn is_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn id(value: &str) -> Result<()> {
    if is_id(value) {
        Ok(())
    } else {
        fail(Code::Malformed, "a thread or send ID is 32 lowercase hex")
    }
}

fn title(value: &str) -> Result<()> {
    if value.len() > MAX_TITLE || value.chars().any(char::is_control) {
        return fail(Code::Bounds, "thread title exceeds its bound");
    }
    Ok(())
}

impl ThreadCoder {
    fn validate(&self) -> Result<()> {
        crate::protocol::public(&self.host)?;
        crate::protocol::identity(&self.task).map_err(crate::Error::from)?;
        if self
            .project
            .as_ref()
            .is_some_and(|project| project.len() > 128 || project.chars().any(char::is_control))
        {
            return fail(Code::Bounds, "Coder project exceeds its bound");
        }
        Ok(())
    }
}

impl ThreadRow {
    pub(crate) fn validate(&self) -> Result<()> {
        id(&self.thread)?;
        title(&self.title)?;
        self.coder.as_ref().map_or(Ok(()), ThreadCoder::validate)
    }
}

impl ThreadPage {
    pub(crate) fn validate(&self) -> Result<()> {
        id(&self.thread)?;
        title(&self.title)?;
        if self.turns.len() > MAX_TURNS
            || self.start.saturating_add(self.turns.len() as u64) > self.total
        {
            return fail(Code::Bounds, "thread page exceeds its bound");
        }
        for turn in &self.turns {
            if let Some(request) = &turn.request {
                id(request)?;
            }
            if turn.model.as_ref().is_some_and(|model| model.len() > 256) {
                return fail(Code::Bounds, "model name exceeds its bound");
            }
        }
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.len() > MAX_FAILURE)
        {
            return fail(Code::Bounds, "failure exceeds its bound");
        }
        self.coder.as_ref().map_or(Ok(()), ThreadCoder::validate)
    }
}

/// Check a `thread.send` message: not blank, and at most [`MAX_MESSAGE`].
pub(crate) fn message(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > MAX_MESSAGE {
        return fail(Code::Bounds, "the message is empty or too long");
    }
    Ok(())
}

/// Fit `page` inside [`MAX_PAGE_BYTES`] and [`MAX_TURNS`]: drop its oldest
/// turns first, then shorten the one turn left and the streaming reply,
/// keeping their ends. A host calls this before it answers.
pub fn fit(page: &mut ThreadPage) {
    let size = |page: &ThreadPage| serde_json::to_vec(page).map_or(usize::MAX, |bytes| bytes.len());
    while page.turns.len() > MAX_TURNS {
        page.turns.remove(0);
        page.start += 1;
    }
    while size(page) > MAX_PAGE_BYTES && page.turns.len() > 1 {
        page.turns.remove(0);
        page.start += 1;
    }
    if let Some(failure) = page.failure.as_mut() {
        shorten(failure, MAX_FAILURE);
    }
    // A single turn or reply larger than a page keeps its ending, which a
    // reader sees first.
    let budget = MAX_PAGE_BYTES / 3;
    if size(page) > MAX_PAGE_BYTES {
        shorten(&mut page.partial, budget);
    }
    if size(page) > MAX_PAGE_BYTES
        && let Some(turn) = page.turns.first_mut()
    {
        shorten(&mut turn.text, budget);
    }
}

/// Keep at most `max` bytes of `text`'s end, on a character boundary, and
/// mark the cut.
fn shorten(text: &mut String, max: usize) {
    if text.len() <= max {
        return;
    }
    let mut cut = text.len() - max;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    *text = format!("…{}", &text[cut..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(text: &str) -> ThreadTurn {
        ThreadTurn {
            role: ThreadRole::Assistant,
            text: text.into(),
            at: Some(1),
            stopped: false,
            model: None,
            request: None,
        }
    }

    #[test]
    fn a_page_fits_its_bound_keeping_the_newest_turns() {
        let mut page = ThreadPage {
            thread: "a".repeat(32),
            title: "Rain".into(),
            start: 0,
            total: 200,
            turns: (0..200)
                .map(|n| turn(&format!("{n} {}", "x".repeat(900))))
                .collect(),
            busy: true,
            partial: "é".repeat(40_000),
            failure: None,
            coder: None,
        };
        fit(&mut page);
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_PAGE_BYTES);
        assert_eq!(page.start + page.turns.len() as u64, 200);
        assert!(page.turns.last().unwrap().text.starts_with("199 "));
        assert!(page.validate().is_ok());

        let mut one = ThreadPage {
            turns: vec![turn(&"y".repeat(200_000))],
            start: 0,
            total: 1,
            partial: String::new(),
            ..page
        };
        fit(&mut one);
        assert!(serde_json::to_vec(&one).unwrap().len() <= MAX_PAGE_BYTES);
        assert!(one.turns[0].text.starts_with('…'));
    }

    #[test]
    fn ids_and_bounds_are_checked() {
        assert!(is_id(&"0f".repeat(16)));
        assert!(!is_id(&"0F".repeat(16)));
        assert!(!is_id(&"a".repeat(64)));
        let row = ThreadRow {
            thread: "b".repeat(32),
            title: "bad\ntitle".into(),
            started: 1,
            updated: 2,
            pinned: false,
            coder: None,
        };
        assert!(row.validate().is_err());
        assert!(message("  ").is_err());
        assert!(message(&"z".repeat(MAX_MESSAGE + 1)).is_err());
    }
}
