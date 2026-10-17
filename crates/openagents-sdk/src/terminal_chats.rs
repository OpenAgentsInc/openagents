//! Saved terminal chats use the same account history and continuation IDs.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CancellationToken, ChatRef, Client, Error, Session};

/// A terminal chat to save: its computer, title, and messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub computer: String,
    pub title: String,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_unix: Option<u64>,
}

/// Who wrote a terminal chat message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Thinking,
    Tool,
}

/// One message of a terminal chat.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Value>,
}

/// The saved chat, and whether this save changed it.
#[derive(Clone, Debug)]
pub struct Saved {
    pub chat: ChatRef,
    pub changed: bool,
}

#[derive(Deserialize)]
struct Receipt {
    chat: String,
    changed: bool,
}

/// Publish an already screened transcript. The returned ID is also its Loro room
/// ID; callers must not derive a separate ID from the terminal's local filename.
pub async fn save(
    client: &Client,
    session: &Session,
    local_id: &str,
    snapshot: &Snapshot,
    cancel: &CancellationToken,
) -> Result<Saved, Error> {
    client.check_session(session)?;
    if local_id.is_empty()
        || local_id.len() > 128
        || !local_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(Error::InvalidReference);
    }
    if snapshot.computer.is_empty()
        || snapshot.computer.len() > 256
        || snapshot.title.is_empty()
        || snapshot.title.len() > 1024
        || snapshot.messages.len() > 4000
        || snapshot
            .messages
            .iter()
            .any(|m| m.text.len() > 64 * 1024 + 4)
    {
        return Err(Error::TooLarge);
    }
    let body = serde_json::to_value(snapshot).map_err(|_| Error::InvalidResponse)?;
    if serde_json::to_vec(&body)
        .map_err(|_| Error::InvalidResponse)?
        .len()
        > 7 * 1024 * 1024
    {
        return Err(Error::TooLarge);
    }
    let segments = ["threads", "synced", local_id];
    let receipt: Receipt = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(Error::Cancelled),
        result = client.request(Method::PUT, &segments, &[], Some(body), Some(session.token())) => result?,
    };
    let chat = client.chat(session, &receipt.chat)?;
    Ok(Saved {
        chat,
        changed: receipt.changed,
    })
}
