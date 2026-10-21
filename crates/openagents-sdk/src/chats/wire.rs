//! What the chat API sends and takes (`/v1/threads`, its messages, and its
//! event stream), with the rules every client applies to it. Nothing here
//! makes a request, so the desktop and phone apps (through [`crate::chats`]'s
//! calls) and the website's GPUI chat (through the browser's `fetch` and
//! `EventSource`) read the same values the same way.
//!
//! Rows are read leniently: a field the service adds later is ignored, and
//! one it leaves out, or sends as `null`, takes its default.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// The longest message, in characters, a turn may send.
pub const MAX_MESSAGE_CHARS: usize = 4000;
/// The longest chat title, in characters.
pub const MAX_TITLE_CHARS: usize = 120;
/// The highest message index a turn's events may name.
pub const MAX_EVENT_INDEX: usize = 4000;

/// Why the service refused or ended a turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    QuotaExhausted,
    RateLimited,
    Busy,
    Unavailable,
    Timeout,
    AccessLost,
    StreamTimeout,
    #[serde(untagged)]
    Other(String),
}
/// A refused turn: its code, a message to show, and when to try again.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}
/// One event of a turn's stream: message text, tool output, cards, the end, or an error.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Text {
        request_id: String,
        index: usize,
        role: crate::MessageRole,
        text: String,
        replace: bool,
    },
    Tool {
        request_id: String,
        index: usize,
        role: crate::MessageRole,
        text: String,
        replace: bool,
    },
    Cards {
        request_id: String,
        cards: Value,
    },
    Done {
        request_id: String,
        outcome: String,
        revision: u64,
        /// The answering model's family, one word from the site's fixed
        /// list (`gemini`, `deepseek`, ..., `other`); absent from older
        /// servers and from answers with no named model (#12439).
        #[serde(default)]
        model_family: Option<String>,
    },
    Error {
        request_id: String,
        error: Refusal,
    },
}
impl Event {
    /// The request this event belongs to.
    pub fn request_id(&self) -> &str {
        match self {
            Self::Text { request_id, .. }
            | Self::Tool { request_id, .. }
            | Self::Cards { request_id, .. }
            | Self::Done { request_id, .. }
            | Self::Error { request_id, .. } => request_id,
        }
    }
    /// Reads one event's `data` for the turn `request_id`. Refuses an event
    /// that isn't JSON, belongs to another turn, or names a message past
    /// [`MAX_EVENT_INDEX`].
    pub fn read(data: &str, request_id: &str) -> Result<Self, BadEvent> {
        let event: Self = serde_json::from_str(data).map_err(|_| BadEvent)?;
        if event.request_id() != request_id {
            return Err(BadEvent);
        }
        if matches!(&event, Self::Text { index, .. } | Self::Tool { index, .. } if *index >= MAX_EVENT_INDEX)
        {
            return Err(BadEvent);
        }
        Ok(event)
    }
    /// Whether the turn's stream ends after this event.
    pub fn ends(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Error { .. })
    }
    /// The cards a [`Event::Cards`] carries, each one card's object.
    pub fn card_list(&self) -> Vec<Value> {
        match self {
            Self::Cards { cards, .. } => cards["cards"].as_array().cloned().unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}
/// An event [`Event::read`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadEvent;

/// Splits a turn's event stream into the `data` of each event, as the bytes
/// arrive. The native client reads the response through it; a browser's
/// `EventSource` does the same splitting itself.
#[derive(Default)]
pub struct Frames {
    buffer: Vec<u8>,
}
impl Frames {
    /// The most a stream may hold unread.
    pub const LIMIT: usize = 17 * 1024 * 1024;
    /// Adds bytes read from the stream. Refuses more than [`Frames::LIMIT`] unread.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), TooLarge> {
        if chunk.len() > Self::LIMIT - self.buffer.len() {
            return Err(TooLarge);
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }
    /// The next whole event's `data` lines, joined, or `None` until more
    /// bytes arrive. An event with no data (a comment or keepalive) is skipped.
    pub fn next(&mut self) -> Result<Option<String>, BadEvent> {
        loop {
            let Some((end, separator)) = self
                .buffer
                .windows(2)
                .position(|w| w == b"\n\n")
                .map(|p| (p, 2))
                .or_else(|| {
                    self.buffer
                        .windows(4)
                        .position(|w| w == b"\r\n\r\n")
                        .map(|p| (p, 4))
                })
            else {
                return Ok(None);
            };
            let raw: Vec<u8> = self.buffer.drain(..end + separator).collect();
            let frame = std::str::from_utf8(&raw).map_err(|_| BadEvent)?;
            let data = frame
                .lines()
                .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                return Ok(Some(data));
            }
        }
    }
}
impl Drop for Frames {
    fn drop(&mut self) {
        self.buffer.fill(0);
    }
}
/// A stream that sent more than [`Frames::LIMIT`] without ending an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooLarge;

/// Whether `id` is a request ID the service takes: a lowercase or uppercase
/// hex UUID with its dashes.
pub fn is_request_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
/// Whether `text` may be sent: not blank, and at most [`MAX_MESSAGE_CHARS`].
pub fn message_ok(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= MAX_MESSAGE_CHARS
}
/// `title` trimmed, when it may name a chat: not blank, at most
/// [`MAX_TITLE_CHARS`], and without control characters.
pub fn title(title: &str) -> Option<&str> {
    let title = title.trim();
    (!title.is_empty()
        && title.chars().count() <= MAX_TITLE_CHARS
        && !title.chars().any(char::is_control))
    .then_some(title)
}

/// A field the service may send as `null`: read as its default.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One chat in `GET /v1/threads`, and the row of `GET /v1/threads/{id}`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Thread {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The last message's request ID.
    pub request_id: Option<String>,
    /// That request's outcome: `pending`, `answered`, `failed`, `unknown`,
    /// or `stopped`.
    pub outcome: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    /// The row's second line.
    #[serde(deserialize_with = "nullable")]
    pub line: String,
    /// `web`, `terminal`, or `phone`.
    #[serde(deserialize_with = "nullable")]
    pub surface: String,
    /// The computer a terminal chat runs on.
    pub computer: Option<String>,
    /// A terminal chat's Coder session, which its running work names.
    pub session: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub updated_unix: u64,
    /// An answer is being written.
    #[serde(deserialize_with = "nullable")]
    pub working: bool,
    /// A terminal chat's computer checked in recently.
    #[serde(deserialize_with = "nullable")]
    pub online: bool,
    /// The signed-in person may continue the chat here.
    #[serde(deserialize_with = "nullable")]
    pub can_reply: bool,
    #[serde(deserialize_with = "nullable")]
    pub pinned: bool,
    /// The project it's filed under, by ID.
    pub project: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub messages: usize,
}
impl Thread {
    /// Whether an answer is being written now.
    pub fn pending(&self) -> bool {
        self.outcome.as_deref() == Some("pending")
    }
    /// Whether the chat lives on a computer or phone rather than here.
    pub fn synced(&self) -> bool {
        self.surface != "web" && !self.surface.is_empty()
    }
}
/// `GET /v1/threads`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadList {
    #[serde(deserialize_with = "nullable")]
    pub threads: Vec<Thread>,
    #[serde(deserialize_with = "nullable")]
    pub total: usize,
    #[serde(deserialize_with = "nullable")]
    pub more: bool,
    pub next_offset: Option<usize>,
}
/// A file sent with a message.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FileRef {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    /// `image`, `pdf`, or `text`.
    #[serde(deserialize_with = "nullable")]
    pub kind: String,
    #[serde(deserialize_with = "nullable")]
    pub size: u64,
}
/// One message in `GET /v1/threads/{id}`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ThreadMessage {
    #[serde(deserialize_with = "nullable")]
    pub role: String,
    #[serde(deserialize_with = "nullable")]
    pub text: String,
    #[serde(deserialize_with = "nullable")]
    pub files: Vec<FileRef>,
}
/// `GET /v1/threads/{id}`: the row and its newest messages, empty ones left out.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadDetail {
    #[serde(deserialize_with = "nullable")]
    pub thread: Thread,
    #[serde(deserialize_with = "nullable")]
    pub messages: Vec<ThreadMessage>,
    /// How many earlier messages the read left out.
    #[serde(deserialize_with = "nullable")]
    pub earlier: usize,
}
/// One message in `GET /v1/threads/{id}/messages`, which keeps every
/// message and its position (`msg_{index}`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MessageRow {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub role: String,
    #[serde(deserialize_with = "nullable")]
    pub content: Vec<Content>,
}
/// One part of a [`MessageRow`].
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Content {
    /// `text`, or a kind this SDK doesn't read.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "nullable")]
    pub text: String,
}
impl MessageRow {
    /// Its position in the chat.
    pub fn index(&self) -> Option<usize> {
        self.id.strip_prefix("msg_")?.parse().ok()
    }
    /// Its text parts, joined by line breaks.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter(|c| c.kind == "text")
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
/// A page of `{data, next}`.
#[derive(Clone, Debug, Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
pub struct Page<T> {
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    /// The cursor for the next page, when there is one.
    #[serde(default)]
    pub next: Option<String>,
}
/// The body of `POST /v1/threads` and `POST /v1/threads/{id}/messages`.
#[derive(Clone, Debug, Serialize)]
pub struct MessageBody<'a> {
    pub request_id: &'a str,
    pub text: &'a str,
    /// The project a new chat starts in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<&'a str>,
}
/// `201 {id, answering}` from `POST /v1/threads`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Started {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub answering: bool,
}
/// The body of `PATCH /v1/threads/{id}`: only what changes.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Change {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    /// `Some(None)` takes it out of its project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<Option<String>>,
}
/// `GET /v1/agents`: the running work Coder reports, by computer.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Boards {
    #[serde(deserialize_with = "nullable")]
    pub computers: Vec<Board>,
}
/// One computer's running work.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Board {
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub online: bool,
    #[serde(deserialize_with = "nullable")]
    pub items: Vec<BoardItem>,
}
/// One piece of running work: a chat's turn or a background agent.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BoardItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// `chat`, `agent`, or `landing`.
    #[serde(deserialize_with = "nullable")]
    pub kind: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    /// `working`, `asking`, `done`, `failed`, or `stopped`.
    #[serde(deserialize_with = "nullable")]
    pub status: String,
    /// The Coder session it belongs to.
    pub session: Option<String>,
    /// What it waits on while `asking`.
    pub question: Option<ItemQuestion>,
    /// Its latest line.
    pub line: Option<String>,
}
/// A question running work waits on.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ItemQuestion {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub text: String,
}
impl Boards {
    /// The question the chat with Coder session `session` waits on: its
    /// computer, the item, and the question.
    pub fn asking(&self, session: &str) -> Option<(&str, &BoardItem, &ItemQuestion)> {
        self.computers.iter().find_map(|board| {
            board.items.iter().find_map(|item| {
                let mine = item.id == session || item.session.as_deref() == Some(session);
                let question = item.question.as_ref()?;
                (item.kind == "chat" && item.status == "asking" && mine).then_some((
                    board.name.as_str(),
                    item,
                    question,
                ))
            })
        })
    }
    /// The background agents of the chat with Coder session `session`,
    /// with their computer.
    pub fn agents(&self, session: &str) -> Vec<(&str, &BoardItem)> {
        self.computers
            .iter()
            .flat_map(|board| {
                board
                    .items
                    .iter()
                    .map(move |item| (board.name.as_str(), item))
            })
            .filter(|(_, item)| item.kind == "agent" && item.session.as_deref() == Some(session))
            .collect()
    }
}
/// The body of `POST /v1/agents/actions`: Stop, Approve, Deny, or Message.
#[derive(Clone, Debug, Serialize)]
pub struct ItemAction<'a> {
    pub request_id: &'a str,
    pub computer: &'a str,
    pub item: &'a str,
    /// `stop`, `approve`, `deny`, or `message`.
    pub action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<&'a str>,
}

/// The service's answer to a stop request.
#[derive(Clone, Debug, Deserialize)]
pub struct Stopped {
    pub request_id: String,
    pub stopped: bool,
    pub finished: bool,
}
/// Whether a chat is pinned and archived, as the service reports it after
/// a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filing {
    /// The chat is pinned to the top of the list.
    pub pinned: bool,
    /// The chat is archived: out of the list until restored.
    pub archived: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0b3f6a52-8f0c-4b7e-9d55-0d6c1d2e3f40";

    #[test]
    fn events_read_for_their_turn_only() {
        let text = format!(
            r#"{{"type":"text","request_id":"{ID}","index":3,"role":"assistant","text":"He","replace":true,"revision":2}}"#
        );
        let event = Event::read(&text, ID).unwrap();
        assert!(
            matches!(&event, Event::Text { index: 3, text, replace: true, .. } if text == "He")
        );
        assert!(!event.ends());
        assert_eq!(Event::read(&text, "other").unwrap_err(), BadEvent);
        let far = text.replace("\"index\":3", "\"index\":4000");
        assert_eq!(Event::read(&far, ID).unwrap_err(), BadEvent);
        let done =
            format!(r#"{{"type":"done","request_id":"{ID}","outcome":"answered","revision":4}}"#);
        assert!(Event::read(&done, ID).unwrap().ends());
        let cards = format!(
            r#"{{"type":"cards","request_id":"{ID}","cards":{{"cards":[{{"card":"video_lab"}}]}}}}"#
        );
        assert_eq!(Event::read(&cards, ID).unwrap().card_list().len(), 1);
        let error = format!(
            r#"{{"type":"error","request_id":"{ID}","error":{{"code":"busy","message":"Wait.","retry_after_ms":null}}}}"#
        );
        assert!(
            matches!(Event::read(&error, ID).unwrap(), Event::Error { error, .. } if error.code == RefusalCode::Busy)
        );
    }

    #[test]
    fn frames_split_on_blank_lines_and_skip_keepalives() {
        let mut frames = Frames::default();
        frames
            .push(b": keepalive\n\nevent: text\ndata: {\"a\":")
            .unwrap();
        assert_eq!(frames.next().unwrap(), None);
        frames.push(b"1}\n\r\n\r\ndata: x\r\n\r\n").unwrap();
        assert_eq!(frames.next().unwrap().as_deref(), Some("{\"a\":1}"));
        assert_eq!(frames.next().unwrap().as_deref(), Some("x"));
        assert_eq!(frames.next().unwrap(), None);
    }

    #[test]
    fn rules_for_ids_messages_and_titles() {
        assert!(is_request_id(ID));
        assert!(!is_request_id("not-a-uuid"));
        assert!(message_ok("hi"));
        assert!(!message_ok("  "));
        assert!(!message_ok(&"x".repeat(MAX_MESSAGE_CHARS + 1)));
        assert_eq!(title("  Plans "), Some("Plans"));
        assert_eq!(title("a\u{7}b"), None);
        assert_eq!(title(&"x".repeat(MAX_TITLE_CHARS + 1)), None);
    }

    #[test]
    fn a_thread_row_reads_with_missing_and_null_fields() {
        let row: Thread = serde_json::from_str(
            r#"{"id":"c1","title":null,"surface":"web","outcome":"pending","pinned":true,"extra":1}"#,
        )
        .unwrap();
        assert!(row.pending());
        assert!(row.pinned);
        assert!(!row.synced());
        assert_eq!(row.title, "");
        assert_eq!(row.project, None);
    }

    #[test]
    fn boards_find_a_chat_question_and_its_agents() {
        let boards: Boards = serde_json::from_str(
            r#"{"computers":[{"name":"mac","online":true,"items":[
                {"id":"s1","kind":"chat","title":"t","status":"asking","question":{"id":"q1","text":"Deploy?"}},
                {"id":"a1","kind":"agent","title":"fix","status":"working","session":"s1","line":"tests"},
                {"id":"a2","kind":"agent","title":"other","status":"working","session":"s2"}]}]}"#,
        )
        .unwrap();
        let (computer, item, question) = boards.asking("s1").unwrap();
        assert_eq!(
            (computer, item.id.as_str(), question.id.as_str()),
            ("mac", "s1", "q1")
        );
        assert!(boards.asking("s2").is_none());
        let agents = boards.agents("s1");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].1.id, "a1");
        let action = ItemAction {
            request_id: "r",
            computer: "mac",
            item: "s1",
            action: "approve",
            question: Some("q1"),
            text: None,
        };
        assert_eq!(
            serde_json::to_string(&action).unwrap(),
            r#"{"request_id":"r","computer":"mac","item":"s1","action":"approve","question":"q1"}"#
        );
    }

    #[test]
    fn message_rows_know_their_index() {
        let row: MessageRow = serde_json::from_str(
            r#"{"id":"msg_12","role":"assistant","content":[{"type":"text","text":"a"}]}"#,
        )
        .unwrap();
        assert_eq!(row.index(), Some(12));
        assert_eq!(row.text(), "a");
    }

    #[test]
    fn a_change_sends_only_what_changed() {
        let change = Change {
            title: Some("New".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&change).unwrap(),
            r#"{"title":"New"}"#
        );
        let out = Change {
            project: Some(None),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&out).unwrap(), r#"{"project":null}"#);
    }
}
