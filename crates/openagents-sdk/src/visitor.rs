//! Chats before sign-in (#11978): a person who just installed an app talks
//! to OpenAgents for a few messages, then signs in, and [`claim`] moves those
//! chats to their account.
//!
//! A [`Visitor`] is a server-signed token, kept by the app like a credential.
//! Its chats are words only. When its messages are used up, sending fails
//! with `Error::Refused { code: Some("sign_in_required"), .. }`.
use std::fmt;

use reqwest::Method;
use serde_json::{Value, json};

use crate::chats::{Stopped, Stream, new_id, uuid};
use crate::{CancellationToken, Client, Deployment, Error, Session};

/// A signed-out visitor on one deployment.
#[derive(Clone, PartialEq, Eq)]
pub struct Visitor {
    deployment: Deployment,
    token: String,
}

impl fmt::Debug for Visitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Visitor([redacted])")
    }
}

impl Visitor {
    /// Restore a visitor the app kept. The server checks the token on use.
    pub fn restore(deployment: Deployment, token: String) -> Result<Self, Error> {
        if !token.starts_with("ovis_")
            || token.len() > 128
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::InvalidCredential);
        }
        Ok(Self { deployment, token })
    }
    /// The token, for the app's own credential store only.
    pub fn token(&self) -> &str {
        &self.token
    }
    /// The deployment the visitor belongs to.
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }
    fn check(&self, client: &Client) -> Result<(), Error> {
        if &self.deployment != client.deployment() {
            return Err(Error::WrongDeployment);
        }
        Ok(())
    }
}

/// A new visitor: `POST /v1/visitors`. Also returns how many messages it may send.
pub async fn begin(client: &Client, cancel: &CancellationToken) -> Result<(Visitor, u32), Error> {
    let future = async {
        client
            .request::<Value>(Method::POST, &["visitors"], &[], None, None)
            .await
    };
    let value =
        tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    let token = value["token"].as_str().ok_or(Error::InvalidResponse)?;
    let messages = value["messages"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(Error::InvalidResponse)?;
    let visitor = Visitor::restore(client.deployment().clone(), token.to_owned())
        .map_err(|_| Error::InvalidResponse)?;
    Ok((visitor, messages))
}

/// One message, kept so a retry after an uncertain response is sent once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    chat: String,
    request_id: String,
    text: String,
    first: bool,
}

impl Message {
    /// The first message of a new chat; the chat is named by its request id.
    pub fn first(text: String) -> Result<Self, Error> {
        let id = new_id()?;
        Self::build(id.clone(), id, text, true)
    }
    /// A reply in chat `chat`.
    pub fn reply(chat: String, text: String) -> Result<Self, Error> {
        Self::build(chat, new_id()?, text, false)
    }
    fn build(chat: String, request_id: String, text: String, first: bool) -> Result<Self, Error> {
        if !uuid(&chat) || text.trim().is_empty() || text.chars().count() > 4000 {
            return Err(Error::InvalidReference);
        }
        Ok(Self {
            chat,
            request_id,
            text,
            first,
        })
    }
    /// The same message under an already-sent request id, to stop or follow it.
    pub fn with_request(mut self, request_id: String) -> Self {
        if uuid(&request_id) {
            self.request_id = request_id;
        }
        self
    }
    /// The chat this message is in.
    pub fn chat(&self) -> &str {
        &self.chat
    }
    /// The message's request ID.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    /// The message text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Send `message`. Returns how many messages the visitor has left after it.
pub async fn send(
    client: &Client,
    visitor: &Visitor,
    message: &Message,
    cancel: &CancellationToken,
) -> Result<u32, Error> {
    visitor.check(client)?;
    let body = json!({"request_id": message.request_id, "text": message.text});
    let future = async {
        if message.first {
            client
                .request::<Value>(
                    Method::POST,
                    &["threads"],
                    &[],
                    Some(body),
                    Some(&visitor.token),
                )
                .await
        } else {
            client
                .request::<Value>(
                    Method::POST,
                    &["threads", &message.chat, "messages"],
                    &[],
                    Some(body),
                    Some(&visitor.token),
                )
                .await
        }
    };
    let value =
        tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    if value["answering"] != true {
        return Err(Error::InvalidResponse);
    }
    value["left"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(Error::InvalidResponse)
}

/// Follow the answer to `message`. Reconnecting starts with replacements.
pub async fn stream(
    client: &Client,
    visitor: &Visitor,
    message: &Message,
    cancel: &CancellationToken,
) -> Result<Stream, Error> {
    visitor.check(client)?;
    crate::chats::events(
        client,
        &message.chat,
        &message.request_id,
        &visitor.token,
        cancel,
    )
    .await
}

/// Stop the answer to `message` only.
pub async fn stop(
    client: &Client,
    visitor: &Visitor,
    message: &Message,
    cancel: &CancellationToken,
) -> Result<Stopped, Error> {
    visitor.check(client)?;
    let future = async {
        client
            .request::<Stopped>(
                Method::POST,
                &["threads", &message.chat, "stop"],
                &[],
                Some(json!({"request_id": message.request_id})),
                Some(&visitor.token),
            )
            .await
    };
    let result =
        tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    if result.request_id != message.request_id || !result.finished {
        return Err(Error::InvalidResponse);
    }
    Ok(result)
}

/// One message of a visitor's chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub role: crate::MessageRole,
    pub text: String,
}

/// The newest messages of the visitor's chat `chat`.
pub async fn messages(
    client: &Client,
    visitor: &Visitor,
    chat: &str,
    cancel: &CancellationToken,
) -> Result<Vec<Line>, Error> {
    visitor.check(client)?;
    if !uuid(chat) {
        return Err(Error::InvalidReference);
    }
    let future = async {
        client
            .request::<Value>(
                Method::GET,
                &["threads", chat],
                &[],
                None,
                Some(&visitor.token),
            )
            .await
    };
    let value =
        tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    value["messages"]
        .as_array()
        .ok_or(Error::InvalidResponse)?
        .iter()
        .map(|m| {
            Ok(Line {
                role: serde_json::from_value(m["role"].clone())
                    .map_err(|_| Error::InvalidResponse)?,
                text: m["text"].as_str().ok_or(Error::InvalidResponse)?.to_owned(),
            })
        })
        .collect()
}

/// Signed in: move the visitor's chats to the account. Returns how many
/// moved; a chat still being answered stays with the visitor.
pub async fn claim(
    client: &Client,
    session: &Session,
    visitor: &Visitor,
    cancel: &CancellationToken,
) -> Result<u32, Error> {
    visitor.check(client)?;
    client.check_session(session)?;
    let future = async {
        client
            .request::<Value>(
                Method::POST,
                &["visitors", "claim"],
                &[],
                Some(json!({"token": visitor.token})),
                Some(session.token()),
            )
            .await
    };
    let value =
        tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelled),v=future=>v?};
    value["moved"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(Error::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_messages_are_checked_before_sending() {
        let deployment = Deployment::new("https://api.example/v1", "https://example").unwrap();
        assert!(Visitor::restore(deployment.clone(), "sess_x".into()).is_err());
        assert!(Visitor::restore(deployment.clone(), "ovis_a/b".into()).is_err());
        let visitor = Visitor::restore(deployment, "ovis_abc123".into()).unwrap();
        assert_eq!(format!("{visitor:?}"), "Visitor([redacted])");
        let first = Message::first("hello".into()).unwrap();
        assert_eq!(first.chat(), first.request_id());
        let reply = Message::reply(first.chat().into(), "again".into()).unwrap();
        assert_ne!(reply.request_id(), first.request_id());
        assert!(Message::first("  ".into()).is_err());
        assert!(Message::reply("not-a-chat".into(), "x".into()).is_err());
    }
}
