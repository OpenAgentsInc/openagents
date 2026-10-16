//! Cloud turns in the existing account chat. Keep a `Send` to retry the same message.
use crate::{CancellationToken, ChatRef, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}
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
    pub fn new(chat: ChatRef, request_id: String) -> Result<Self, Error> {
        if !uuid(&request_id) {
            return Err(Error::InvalidReference);
        }
        Ok(Self { chat, request_id })
    }
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
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
    pub fn new(chat: ChatRef, text: String) -> Result<Self, Error> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::InvalidResponse)?;
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let id = format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        );
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
    pub fn turn(&self) -> Turn {
        Turn {
            chat: self.chat.clone(),
            request_id: self.request_id.clone(),
        }
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn chat(&self) -> &ChatRef {
        &self.chat
    }
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
fn uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
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
    let segments = ["threads", message.chat.id(), "events"];
    let future = async {
        client
            .send_events(
                &segments,
                &[("request_id", message.request_id.clone())],
                session.token(),
            )
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
        request: message.request_id.clone(),
        cancel: cancel.clone(),
        ended: false,
    })
}
pub struct Stream {
    response: reqwest::Response,
    buffer: Vec<u8>,
    request: String,
    cancel: CancellationToken,
    ended: bool,
}
impl Stream {
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
