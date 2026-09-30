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
//! A turn may carry the router's offers, cards, and follow-up chips
//! ([`ThreadExtras`]). The typed judgment (tier, answer, route, bank, and
//! judgment text) stays on the host. A thread that delegated Coder work
//! names the task ([`ThreadCoder`]); the device reads that task through the
//! observer profile, as it reads any Coder chat. `thread.run` asks the host
//! to start that work through the same handoff the desktop uses.
use crate::{Code, Result, fail};
use serde::{Deserialize, Serialize};

/// The most rows a `threads` outcome carries: the host's newest threads
/// that are not archived.
pub const MAX_THREADS: usize = 128;
/// The most turns one `thread` page carries.
pub const MAX_TURNS: usize = 64;
/// The largest encoded `thread` or `threads` outcome, so a reply seals
/// well inside one relay frame (128 KiB, base64 and the envelope
/// included). A host drops extras on the oldest turns first, then older
/// turns, and shortens a single turn's text, to stay inside it.
pub const MAX_PAGE_BYTES: usize = 48 * 1024;
/// The longest thread title.
pub const MAX_TITLE: usize = 160;
/// The longest message a device sends to a thread, as the chat service
/// allows.
pub const MAX_MESSAGE: usize = 32 * 1024;
/// The longest refusal or failure text a page carries.
pub const MAX_FAILURE: usize = 1024;
/// The most offers one turn carries.
pub const MAX_OFFERS: usize = 4;
/// The most follow-up chips one turn carries.
pub const MAX_FOLLOWUPS: usize = 3;
/// The most cards one turn carries.
pub const MAX_CARDS: usize = 4;
/// The longest follow-up label, in characters.
pub const MAX_FOLLOWUP_CHARS: usize = 80;
/// The largest encoded offer or card object.
pub const MAX_EXTRA_VALUE: usize = 8 * 1024;

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

/// Coder work the thread ran on this computer outside the host: a local
/// run (`openagents chat`) whose task store is not the one the host
/// serves, so no device can open, follow, or stop it through the host. A
/// device says so plainly and offers no Coder control for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadOutside {
    /// The Coder task's ID in that other store.
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

/// One follow-up chip: tapping it sends `label` as the person's message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadFollowup {
    /// The prepared answer it leads to, when the router named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub label: String,
}

/// Offers, follow-up chips, and cards on one turn. Absent on an older page,
/// which means there are none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadExtras {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub followups: Vec<ThreadFollowup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<serde_json::Value>,
}

impl ThreadExtras {
    /// Nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.offers.is_empty() && self.followups.is_empty() && self.cards.is_empty()
    }
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
    /// Offers, follow-up chips, and cards on this turn. Absent on an older
    /// page, which means there are none. The router's typed judgment stays
    /// on the host.
    #[serde(default, skip_serializing_if = "ThreadExtras::is_empty")]
    pub extras: ThreadExtras,
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
    /// Coder work the thread ran on this computer outside the host. Absent
    /// unless the thread names a local run the host's task store does not
    /// hold, so an older page, and every other page, omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outside: Option<ThreadOutside>,
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

impl ThreadOutside {
    fn validate(&self) -> Result<()> {
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
            turn.extras.validate()?;
        }
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.len() > MAX_FAILURE)
        {
            return fail(Code::Bounds, "failure exceeds its bound");
        }
        if self.coder.is_some() && self.outside.is_some() {
            return fail(Code::Malformed, "a thread names one Coder task");
        }
        self.outside
            .as_ref()
            .map_or(Ok(()), ThreadOutside::validate)?;
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

impl ThreadExtras {
    fn validate(&self) -> Result<()> {
        if self.offers.len() > MAX_OFFERS
            || self.followups.len() > MAX_FOLLOWUPS
            || self.cards.len() > MAX_CARDS
        {
            return fail(Code::Bounds, "thread extras exceed their bound");
        }
        for offer in &self.offers {
            extra_value(offer)?;
        }
        for card in &self.cards {
            extra_value(card)?;
        }
        for followup in &self.followups {
            let label = followup.label.chars().count();
            if !(1..=MAX_FOLLOWUP_CHARS).contains(&label)
                || followup.label.chars().any(char::is_control)
            {
                return fail(Code::Bounds, "a follow-up label exceeds its bound");
            }
            if followup
                .answer
                .as_ref()
                .is_some_and(|answer| !tag_like(answer))
            {
                return fail(Code::Malformed, "a follow-up answer exceeds its bound");
            }
        }
        Ok(())
    }
}

/// An offer or card body: one JSON object, at most [`MAX_EXTRA_VALUE`].
fn extra_value(value: &serde_json::Value) -> Result<()> {
    if !value.is_object() {
        return fail(Code::Malformed, "an offer or card is an object");
    }
    if serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MAX_EXTRA_VALUE) {
        return fail(Code::Bounds, "an offer or card exceeds its bound");
    }
    Ok(())
}

/// A short bank id: lowercase ASCII, digits, and `._@-:` .
fn tag_like(text: &str) -> bool {
    (1..=96).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._@-:".contains(&byte)
        })
}

/// Fit `page` inside [`MAX_PAGE_BYTES`] and [`MAX_TURNS`]: drop extras on
/// the oldest turns first, then drop those turns, then shorten the one
/// turn left and the streaming reply, keeping their ends. A host calls
/// this before it answers.
pub fn fit(page: &mut ThreadPage) {
    let size = |page: &ThreadPage| serde_json::to_vec(page).map_or(usize::MAX, |bytes| bytes.len());
    while page.turns.len() > MAX_TURNS {
        page.turns.remove(0);
        page.start += 1;
    }
    while size(page) > MAX_PAGE_BYTES {
        let Some(turn) = page.turns.iter_mut().find(|turn| !turn.extras.is_empty()) else {
            break;
        };
        turn.extras = ThreadExtras::default();
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
            extras: ThreadExtras::default(),
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
            outside: None,
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

    #[test]
    fn extras_drop_before_turns_and_an_older_page_has_none() {
        let bulky = |byte: char| ThreadExtras {
            cards: vec![serde_json::json!({"card": byte.to_string().repeat(26_000)})],
            ..ThreadExtras::default()
        };
        let mut older = turn("kept older");
        older.extras = bulky('a');
        let mut newer = turn("kept newer");
        newer.extras = bulky('b');
        let mut page = ThreadPage {
            thread: "c".repeat(32),
            title: "Rain".into(),
            start: 0,
            total: 2,
            turns: vec![older, newer],
            busy: false,
            partial: String::new(),
            failure: None,
            coder: None,
            outside: None,
        };
        fit(&mut page);
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_PAGE_BYTES);
        assert_eq!(page.turns.len(), 2);
        assert!(page.turns[0].extras.is_empty());
        assert_eq!(page.turns[0].text, "kept older");
        assert!(!page.turns[1].extras.is_empty());
        assert_eq!(page.turns[1].text, "kept newer");

        let mut huge = turn("plain words");
        huge.extras
            .cards
            .push(serde_json::json!({"card": "z".repeat(60_000)}));
        let mut one = ThreadPage {
            turns: vec![huge],
            start: 0,
            total: 1,
            partial: String::new(),
            ..page
        };
        fit(&mut one);
        assert!(one.turns[0].extras.is_empty());
        assert_eq!(one.turns[0].text, "plain words");
        assert!(serde_json::to_vec(&one).unwrap().len() <= MAX_PAGE_BYTES);

        let bare = serde_json::to_value(turn("Hi")).unwrap();
        assert!(bare.get("extras").is_none());
        let old: ThreadTurn = serde_json::from_value(bare).unwrap();
        assert!(old.extras.is_empty());

        let mut too_many = turn("bounds");
        too_many.extras.offers = (0..5).map(|n| serde_json::json!({"n": n})).collect();
        let over = ThreadPage {
            turns: vec![too_many],
            total: 1,
            ..one
        };
        assert!(over.validate().is_err());
    }

    #[test]
    fn a_local_run_outside_the_host_is_named_only_when_present() {
        let page = ThreadPage {
            thread: "c".repeat(32),
            title: "Rain".into(),
            start: 0,
            total: 0,
            turns: vec![],
            busy: false,
            partial: String::new(),
            failure: None,
            coder: None,
            outside: None,
        };
        // Every page without an outside run encodes as an older page did,
        // and an older page decodes with none.
        let bare = serde_json::to_value(&page).unwrap();
        assert!(bare.get("outside").is_none());
        let old: ThreadPage = serde_json::from_value(bare).unwrap();
        assert_eq!(old, page);

        let outside = ThreadPage {
            outside: Some(ThreadOutside {
                task: "d".repeat(64),
                project: Some("checkout".into()),
                at: Some(1),
            }),
            ..page.clone()
        };
        assert!(outside.validate().is_ok());
        let back: ThreadPage =
            serde_json::from_value(serde_json::to_value(&outside).unwrap()).unwrap();
        assert_eq!(back, outside);
        let both = ThreadPage {
            coder: Some(ThreadCoder {
                host: "e".repeat(64),
                task: "d".repeat(64),
                project: None,
                at: None,
            }),
            ..outside.clone()
        };
        assert!(both.validate().is_err());
        let mut bad = outside;
        bad.outside.as_mut().unwrap().task = "local".into();
        assert!(bad.validate().is_err());
    }
}
