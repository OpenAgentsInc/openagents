//! Cloud turns in the existing account chat. Keep a `Send` to retry the same message.
use crate::{CancellationToken, ChatRef, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
}
/// A cloud turn in one account chat, including turns started on another device.
#[derive(Clone, Debug)]
pub struct Turn {
    chat: ChatRef,
    request_id: String,
}
impl Turn {
    /// Names a turn by its chat and request ID. Refuses a request ID that isn't a UUID.
    pub fn new(chat: ChatRef, request_id: String) -> Result<Self, Error> {
        if !uuid(&request_id) {
            return Err(Error::InvalidReference);
        }
        Ok(Self { chat, request_id })
    }
    /// The chat the turn is in.
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    /// The turn's request ID.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    fn check(&self, client: &Client, session: &Session) -> Result<(), Error> {
        client.check_session(session)?;
        if self.chat.deployment() != client.deployment() {
            return Err(Error::WrongDeployment);
        }
        if self.chat.account() != session.account().id {
            return Err(Error::WrongAccount);
        }
        Ok(())
    }
}
/// The latest turn in a chat, its outcome, and whether it's still working.
pub struct Current {
    pub turn: Turn,
    pub outcome: String,
    pub working: bool,
}
/// Find the latest turn, including a turn sent from the web or another app.
pub async fn current(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    cancel: &CancellationToken,
) -> Result<Option<Current>, Error> {
    Turn {
        chat: chat.clone(),
        request_id: String::new(),
    }
    .check(client, session)?;
    let future = async {
        client
            .request::<Value>(
                Method::GET,
                &["threads", chat.id()],
                &[],
                None,
                Some(session.token()),
            )
            .await
    };
    let value =
        tokio::select! {biased;_=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    let row = &value["thread"];
    let Some(id) = row["request_id"].as_str() else {
        return Ok(None);
    };
    Ok(Some(Current {
        turn: Turn::new(chat.clone(), id.to_owned())?,
        outcome: row["outcome"]
            .as_str()
            .ok_or(Error::InvalidResponse)?
            .to_owned(),
        working: row["working"].as_bool().ok_or(Error::InvalidResponse)?,
    }))
}
/// A retained retry identity, bound to one chat, account and deployment.
#[derive(Clone, Debug)]
pub struct Send {
    chat: ChatRef,
    request_id: String,
    text: String,
}
impl Send {
    /// Creates a message for `chat` with a new request ID. Refuses empty text or text
    /// over 4,000 characters.
    pub fn new(chat: ChatRef, text: String) -> Result<Self, Error> {
        Self::with_id(chat, new_id()?, text)
    }
    /// The first message of a new chat answered by OpenAgents (#11979): the
    /// chat is named by this message's request id. Send it with [`start`].
    pub fn first(client: &Client, session: &Session, text: String) -> Result<Self, Error> {
        let id = new_id()?;
        let chat = client.chat(session, &id)?;
        Self::with_id(chat, id, text)
    }
    /// Restore an explicitly retained request after an uncertain response.
    pub fn with_id(chat: ChatRef, request_id: String, text: String) -> Result<Self, Error> {
        if !uuid(&request_id) || text.trim().is_empty() || text.chars().count() > 4000 {
            return Err(Error::InvalidReference);
        }
        Ok(Self {
            chat,
            request_id,
            text,
        })
    }
    /// The turn this message starts.
    pub fn turn(&self) -> Turn {
        Turn {
            chat: self.chat.clone(),
            request_id: self.request_id.clone(),
        }
    }
    /// The message's request ID. Keep it to retry the same message.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    /// The chat the message goes to.
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
    /// The message text.
    pub fn text(&self) -> &str {
        &self.text
    }
    fn check(&self, client: &Client, session: &Session) -> Result<(), Error> {
        client.check_session(session)?;
        if self.chat.deployment() != client.deployment() {
            return Err(Error::WrongDeployment);
        }
        if self.chat.account() != session.account().id {
            return Err(Error::WrongAccount);
        }
        Ok(())
    }
}
/// A new random request id (a version 4 UUID).
pub(crate) fn new_id() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::InvalidResponse)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}
pub(crate) fn uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
/// Start the new chat [`Send::first`] made (`POST /v1/threads`). Retrying the
/// same `Send` after an uncertain response starts it once.
pub async fn start(
    client: &Client,
    session: &Session,
    message: &Send,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    message.check(client, session)?;
    if message.chat.id() != message.request_id {
        return Err(Error::InvalidReference);
    }
    let future = async {
        client
            .request::<Value>(
                Method::POST,
                &["threads"],
                &[],
                Some(json!({"request_id":message.request_id,"text":message.text})),
                Some(session.token()),
            )
            .await
    };
    let value = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
    if value["answering"] != true || value["id"] != message.request_id.as_str() {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
/// Submit the retained message. Cancellation can leave an accepted turn; retry this same `Send`.
pub async fn send(
    client: &Client,
    session: &Session,
    message: &Send,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    message.check(client, session)?;
    let future = async {
        client
            .request::<Value>(
                Method::POST,
                &["threads", message.chat.id(), "messages"],
                &[],
                Some(json!({"request_id":message.request_id,"text":message.text})),
                Some(session.token()),
            )
            .await
    };
    let value = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
    if value["answering"] != true {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
/// Whether a chat is pinned and archived, as the service reports it after
/// [`set_pinned`] or [`set_archived`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filing {
    /// The chat is pinned to the top of the list.
    pub pinned: bool,
    /// The chat is archived: out of the list until restored.
    pub archived: bool,
}
/// Pin the chat (`true`) or unpin it (`false`), as the web's sidebar does
/// (`PATCH /v1/threads/{id}`). A new pin goes after every earlier one.
/// Repeating the call changes nothing more.
pub async fn set_pinned(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    pinned: bool,
    cancel: &CancellationToken,
) -> Result<Filing, Error> {
    file(client, session, chat, json!({ "pinned": pinned }), cancel).await
}
/// Archive the chat (`true`) or restore it (`false`), as the web's sidebar
/// does (`PATCH /v1/threads/{id}`). Archiving also removes the cloud
/// computers that ran it. Repeating the call changes nothing more.
pub async fn set_archived(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    archived: bool,
    cancel: &CancellationToken,
) -> Result<Filing, Error> {
    file(
        client,
        session,
        chat,
        json!({ "archived": archived }),
        cancel,
    )
    .await
}
/// Rename an owned saved chat. The server applies the same title limits as the web.
pub async fn set_title(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    title: &str,
    cancel: &CancellationToken,
) -> Result<Filing, Error> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 120 || title.chars().any(char::is_control) {
        return Err(Error::InvalidReference);
    }
    file(client, session, chat, json!({"title": title}), cancel).await
}
async fn file(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    body: Value,
    cancel: &CancellationToken,
) -> Result<Filing, Error> {
    Turn {
        chat: chat.clone(),
        request_id: String::new(),
    }
    .check(client, session)?;
    let expected_title = body.get("title").and_then(Value::as_str).map(str::to_owned);
    let future = async {
        client
            .request::<Value>(
                Method::PATCH,
                &["threads", chat.id()],
                &[],
                Some(body),
                Some(session.token()),
            )
            .await
    };
    let value = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
    if value["id"] != chat.id()
        || expected_title
            .as_ref()
            .is_some_and(|title| value["title"].as_str() != Some(title.as_str()))
    {
        return Err(Error::InvalidResponse);
    }
    Ok(Filing {
        pinned: value["pinned"].as_bool().ok_or(Error::InvalidResponse)?,
        archived: value["archived"].as_bool().ok_or(Error::InvalidResponse)?,
    })
}
/// The service's answer to a stop request.
#[derive(Clone, Debug, Deserialize)]
pub struct Stopped {
    pub request_id: String,
    pub stopped: bool,
    pub finished: bool,
}
/// Stop this request only. A late stop never cancels a newer turn.
pub async fn stop(
    client: &Client,
    session: &Session,
    message: &Turn,
    cancel: &CancellationToken,
) -> Result<Stopped, Error> {
    message.check(client, session)?;
    let future = async {
        client
            .request::<Stopped>(
                Method::POST,
                &["threads", message.chat.id(), "stop"],
                &[],
                Some(json!({"request_id":message.request_id})),
                Some(session.token()),
            )
            .await
    };
    let result = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
    if result.request_id != message.request_id || !result.finished {
        return Err(Error::InvalidResponse);
    }
    Ok(result)
}
/// Reconnecting starts with replacements of the current request's messages,
/// then deltas. Apply by message index; never append a replacement twice.
pub async fn stream(
    client: &Client,
    session: &Session,
    message: &Turn,
    cancel: &CancellationToken,
) -> Result<Stream, Error> {
    message.check(client, session)?;
    events(
        client,
        message.chat.id(),
        &message.request_id,
        session.token(),
        cancel,
    )
    .await
}
/// Follow one request's events in chat `chat`, with `token`'s access.
pub(crate) async fn events(
    client: &Client,
    chat: &str,
    request_id: &str,
    token: &str,
    cancel: &CancellationToken,
) -> Result<Stream, Error> {
    let segments = ["threads", chat, "events"];
    let future = async {
        client
            .send_events(&segments, &[("request_id", request_id.to_owned())], token)
            .await
    };
    let response = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),result=future=>result?};
    if !response.status().is_success() {
        Client::decode::<Value>(response, &segments, 1024 * 1024).await?;
        return Err(Error::InvalidResponse);
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| !v.starts_with("text/event-stream"))
    {
        return Err(Error::InvalidResponse);
    }
    Ok(Stream {
        response,
        buffer: Vec::new(),
        request: request_id.to_owned(),
        cancel: cancel.clone(),
        ended: false,
    })
}
/// The events of one turn, read in order with [`Stream::next`].
pub struct Stream {
    response: reqwest::Response,
    buffer: Vec<u8>,
    request: String,
    cancel: CancellationToken,
    ended: bool,
}
impl Stream {
    /// Reads the next event. Returns `None` after the turn's last event.
    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        if self.cancel.is_cancelled() {
            self.buffer.fill(0);
            return Err(Error::Cancelled);
        }
        if self.ended {
            return Ok(None);
        }
        loop {
            if let Some((end, separator)) = self
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
            {
                let raw: Vec<u8> = self.buffer.drain(..end + separator).collect();
                let frame = std::str::from_utf8(&raw).map_err(|_| Error::InvalidResponse)?;
                let data = frame
                    .lines()
                    .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                let event: Event =
                    serde_json::from_str(&data).map_err(|_| Error::InvalidResponse)?;
                if event.request_id() != self.request {
                    return Err(Error::InvalidResponse);
                }
                if matches!(&event,Event::Text{index,..}|Event::Tool{index,..} if *index>=4000) {
                    return Err(Error::InvalidResponse);
                }
                self.ended = matches!(&event, Event::Done { .. } | Event::Error { .. });
                return Ok(Some(event));
            }
            let chunk = tokio::select! {biased; _=self.cancel.cancelled()=>return Err(Error::Cancelled),result=self.response.chunk()=>result.map_err(|e|Error::Network{timeout:e.is_timeout()})?};
            let Some(chunk) = chunk else {
                return Err(Error::InvalidResponse);
            };
            if chunk.len() > 17 * 1024 * 1024 - self.buffer.len() {
                return Err(Error::TooLarge);
            }
            self.buffer.extend_from_slice(&chunk);
        }
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.buffer.fill(0);
    }
}
