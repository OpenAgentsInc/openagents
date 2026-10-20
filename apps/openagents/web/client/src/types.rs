//! What the JSON API answers. The chat API's values are the SDK's own
//! ([`openagents_sdk::chats::wire`], the same ones the desktop and phone
//! apps read), re-exported here; this module adds only what the website
//! alone serves: projects, room tokens, the site's refusal body, and where
//! a page reads a chat's file.

use serde::{Deserialize, Deserializer};

pub use openagents_sdk::MessageRole;
pub use openagents_sdk::chats::wire::{
    BadEvent, Change, Content, Event, FileRef, MessageBody, MessageRow, Page, Started, Thread,
    ThreadDetail, ThreadList, ThreadMessage, is_request_id, message_ok, title,
};
/// Why the service refused or ended a turn, as an [`Event::Error`] carries it.
pub use openagents_sdk::chats::wire::{Refusal as TurnRefusal, RefusalCode};

/// A field the site may send as `null`: read as its default.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Where the page reads `file`, sent in `chat`, signed in by its cookie.
pub fn file_url(chat: &str, file: &FileRef) -> String {
    format!("/chat/{chat}/files/{}", file.id)
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

/// A `coder-sync` room the page may open (#12263): its own registry, a
/// computer's device room as a client, or a chat's room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    Registry,
    Device,
    Chat,
}

impl Room {
    /// The name `POST /v1/sync/room-tokens` takes and the socket path's
    /// first segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Device => "device",
            Self::Chat => "chat2",
        }
    }
}

/// A short-lived token for one room, from `POST /v1/sync/room-tokens`.
/// `url` is the socket address with the token in its query; the socket
/// closes when the token expires, and the caller asks for a new one to
/// reconnect.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RoomToken {
    #[serde(deserialize_with = "nullable")]
    pub token: String,
    #[serde(deserialize_with = "nullable")]
    pub room: String,
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    pub expires_unix: u64,
    #[serde(deserialize_with = "nullable")]
    pub url: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_read_the_site_body() {
        let refusal = Refusal::read(409, br#"{"error":{"code":"busy","message":"Wait."}}"#);
        assert_eq!(refusal.code, "busy");
        assert_eq!(Refusal::read(401, b"").code, "http_401");
    }

    #[test]
    fn projects_read_leniently() {
        let project: Project =
            serde_json::from_str(r#"{"id":"p1","name":null,"role":"viewer","extra":1}"#).unwrap();
        assert_eq!(project.name, "");
        assert!(!project.editable());
    }

    #[test]
    fn a_file_is_read_through_its_chat() {
        let file = FileRef {
            id: "f1".into(),
            ..Default::default()
        };
        assert_eq!(file_url("c1", &file), "/chat/c1/files/f1");
    }
}

#[cfg(test)]
mod room_tests {
    use super::*;

    #[test]
    fn a_room_token_reads_as_the_site_sends_it() {
        let token: RoomToken = serde_json::from_str(
            r#"{"token":"ort1_x","room":"registry","id":"acct_1","expires_unix":300,"url":"wss://sync.openagents.com/registry/acct_1/ws?token=ort1_x"}"#,
        )
        .unwrap();
        assert_eq!(token.id, "acct_1");
        assert_eq!(token.expires_unix, 300);
        assert_eq!(Room::Chat.as_str(), "chat2");
    }
}
