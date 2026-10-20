//! Cloud turns in the existing account chat. Keep a `Send` to retry the same message.
//!
//! [`wire`]'s values and rules need no transport; the calls here are the
//! native client's, and the website's GPUI chat makes the same requests
//! with the browser's own.
pub mod wire;
#[cfg(feature = "transport")]
use crate::{CancellationToken, ChatRef, Client, Error, Session};
#[cfg(feature = "transport")]
use reqwest::Method;
#[cfg(feature = "transport")]
use serde_json::{Value, json};
pub use wire::*;

#[cfg(feature = "transport")]
/// A cloud turn in one account chat, including turns started on another device.
#[derive(Clone, Debug)]
pub struct Turn {
    chat: ChatRef,
    request_id: String,
}
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
/// The latest turn in a chat, its outcome, and whether it's still working.
pub struct Current {
    pub turn: Turn,
    pub outcome: String,
    pub working: bool,
}
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
/// A retained retry identity, bound to one chat, account and deployment.
#[derive(Clone, Debug)]
pub struct Send {
    chat: ChatRef,
    request_id: String,
    text: String,
}
#[cfg(feature = "transport")]
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
        if !uuid(&request_id) || !message_ok(&text) {
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
pub(crate) fn uuid(id: &str) -> bool {
    is_request_id(id)
}
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
/// Rename an owned saved chat. The server applies the same title limits as the web.
pub async fn set_title(
    client: &Client,
    session: &Session,
    chat: &ChatRef,
    title: &str,
    cancel: &CancellationToken,
) -> Result<Filing, Error> {
    let title = self::title(title).ok_or(Error::InvalidReference)?;
    file(client, session, chat, json!({"title": title}), cancel).await
}
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
#[cfg(feature = "transport")]
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
        frames: Frames::default(),
        request: request_id.to_owned(),
        cancel: cancel.clone(),
        ended: false,
    })
}
#[cfg(feature = "transport")]
/// The events of one turn, read in order with [`Stream::next`].
pub struct Stream {
    response: reqwest::Response,
    frames: Frames,
    request: String,
    cancel: CancellationToken,
    ended: bool,
}
#[cfg(feature = "transport")]
impl Stream {
    /// Reads the next event. Returns `None` after the turn's last event.
    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        if self.cancel.is_cancelled() {
            self.frames = Frames::default();
            return Err(Error::Cancelled);
        }
        if self.ended {
            return Ok(None);
        }
        loop {
            if let Some(data) = self.frames.next().map_err(|_| Error::InvalidResponse)? {
                let event =
                    Event::read(&data, &self.request).map_err(|_| Error::InvalidResponse)?;
                self.ended = event.ends();
                return Ok(Some(event));
            }
            let chunk = tokio::select! {biased; _=self.cancel.cancelled()=>return Err(Error::Cancelled),result=self.response.chunk()=>result.map_err(|e|Error::Network{timeout:e.is_timeout()})?};
            let Some(chunk) = chunk else {
                return Err(Error::InvalidResponse);
            };
            self.frames.push(&chunk).map_err(|_| Error::TooLarge)?;
        }
    }
}
