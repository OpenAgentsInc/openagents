//! What the JSON API answers, read leniently: a field the site adds later
//! is ignored, and one it leaves out takes its default.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A field the site may send as `null`: read as its default.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One chat in `GET /v1/threads` (`phone_api::row`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Thread {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The last message's request id.
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
    pub computer: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub updated_unix: u64,
    #[serde(deserialize_with = "nullable")]
    pub working: bool,
    #[serde(deserialize_with = "nullable")]
    pub online: bool,
    #[serde(deserialize_with = "nullable")]
    pub can_reply: bool,
    #[serde(deserialize_with = "nullable")]
    pub pinned: bool,
    /// The project it's filed under, by id.
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

/// A file sent with a message (`chat_files::FileRef`).
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

impl FileRef {
    /// Where the page reads the file, signed in by its cookie.
    pub fn url(&self, chat: &str) -> String {
        format!("/chat/{chat}/files/{}", self.id)
    }
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

/// `GET /v1/threads/{id}`: the row and its newest messages, empty ones left
/// out.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadDetail {
    #[serde(deserialize_with = "nullable")]
    pub thread: Thread,
    #[serde(deserialize_with = "nullable")]
    pub messages: Vec<ThreadMessage>,
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

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Content {
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
    #[serde(default)]
    pub next: Option<String>,
}

/// One project in `GET /v1/projects`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Project {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub repository: Option<String>,
    pub role: Option<String>,
}

impl Project {
    /// Whether the person may file chats into it.
    pub fn editable(&self) -> bool {
        !matches!(self.role.as_deref(), Some("viewer"))
    }
}

/// `POST /v1/threads` and `/v1/threads/{id}/messages`.
#[derive(Clone, Debug, Serialize)]
pub struct Send<'a> {
    pub request_id: &'a str,
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<&'a str>,
}

/// `201 {id, answering}` from `POST /v1/threads`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Started {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
}

/// `PATCH /v1/threads/{id}`.
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

/// A refusal: `{error: {code, message}}`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Refusal {
    #[serde(deserialize_with = "nullable")]
    pub code: String,
    #[serde(deserialize_with = "nullable")]
    pub message: String,
}

impl Refusal {
    /// Reads the site's refusal body, or a plain one for `status`.
    pub fn read(status: u16, body: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct Wrapped {
            error: Refusal,
        }
        serde_json::from_slice::<Wrapped>(body)
            .map(|w| w.error)
            .unwrap_or_else(|_| Refusal {
                code: format!("http_{status}"),
                message: match status {
                    401 => "Sign in again to keep going.".into(),
                    404 => "We couldn't find that chat.".into(),
                    _ => "Something went wrong. Try again.".into(),
                },
            })
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Refusal {}

/// One event of `GET /v1/threads/{id}/events?request_id=` (`chat_api.rs`).
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    /// Text for the message at `index` in the chat: appended, or replacing
    /// what came before when `replace`.
    Text {
        index: usize,
        role: String,
        text: String,
        replace: bool,
    },
    /// The answer's cards.
    Cards(Vec<Value>),
    /// The answer finished: `answered` or `stopped`.
    Done {
        outcome: String,
    },
    Error {
        code: String,
        message: String,
    },
}

impl StreamEvent {
    /// Reads one event from its SSE name and data.
    pub fn parse(name: &str, data: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(data).ok()?;
        let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
        match name {
            "text" | "tool" => Some(StreamEvent::Text {
                index: value["index"].as_u64()? as usize,
                role: text("role"),
                text: text("text"),
                replace: value["replace"].as_bool().unwrap_or(false),
            }),
            "cards" => Some(StreamEvent::Cards(
                value["cards"]["cards"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            )),
            "done" => Some(StreamEvent::Done {
                outcome: text("outcome"),
            }),
            "error" => Some(StreamEvent::Error {
                code: value["error"]["code"]
                    .as_str()
                    .unwrap_or("unavailable")
                    .to_owned(),
                message: value["error"]["message"]
                    .as_str()
                    .unwrap_or("We couldn't get an answer. Try again.")
                    .to_owned(),
            }),
            _ => None,
        }
    }

    /// Whether the stream ends after this event.
    pub fn ends(&self) -> bool {
        matches!(self, StreamEvent::Done { .. } | StreamEvent::Error { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_row_reads_with_missing_fields() {
        let row: Thread = serde_json::from_str(
            r#"{"id":"c1","title":"Hi","surface":"web","outcome":"pending","pinned":true,"extra":1}"#,
        )
        .unwrap();
        assert!(row.pending());
        assert!(row.pinned);
        assert!(!row.synced());
        assert_eq!(row.project, None);
    }

    #[test]
    fn null_fields_read_as_defaults() {
        let row: Thread =
            serde_json::from_str(r#"{"id":"c1","title":null,"line":null,"pinned":null}"#).unwrap();
        assert_eq!(row.line, "");
        assert!(!row.pinned);
    }

    #[test]
    fn stream_events_parse() {
        assert_eq!(
            StreamEvent::parse(
                "text",
                r#"{"type":"text","index":3,"role":"assistant","text":"He","replace":true}"#
            ),
            Some(StreamEvent::Text {
                index: 3,
                role: "assistant".into(),
                text: "He".into(),
                replace: true
            })
        );
        let done = StreamEvent::parse("done", r#"{"outcome":"answered"}"#).unwrap();
        assert!(done.ends());
        let cards =
            StreamEvent::parse("cards", r#"{"cards":{"cards":[{"card":"video_lab"}]}}"#).unwrap();
        assert!(matches!(cards, StreamEvent::Cards(c) if c.len() == 1));
        let error = StreamEvent::parse("error", r#"{"error":{"code":"x","message":"Nope."}}"#);
        assert!(matches!(error, Some(StreamEvent::Error { message, .. }) if message == "Nope."));
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

    #[test]
    fn refusals_read_the_site_body() {
        let refusal = Refusal::read(409, br#"{"error":{"code":"busy","message":"Wait."}}"#);
        assert_eq!(refusal.code, "busy");
        assert_eq!(Refusal::read(401, b"").code, "http_401");
    }
}
