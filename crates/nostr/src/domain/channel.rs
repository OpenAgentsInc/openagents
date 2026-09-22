//! NIP-28 public chat.
//!
//! Five kinds: `40` creates a channel, `41` updates its metadata
//! against an `e` root tag naming the channel, `42` is a message
//! rooted at the channel, `43` hides a message for its author, and
//! `44` mutes a user. Kind `40` and `41` content is JSON with `name`,
//! `about`, `picture`, and `relays`; `43` and `44` may carry a
//! `reason`. Clients SHOULD ignore a kind `41` from a pubkey other
//! than the channel's author — [`metadata_updates_channel`] checks it.
//! NIP-28 is unrecommended — NIP-29 supersedes it — so the kinds are
//! kept for compatibility and not added to the NIP-11 list.

use serde_json::Value;

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

/// A channel's public metadata — the JSON content of kinds `40`/`41`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelMetadata {
    /// The channel name.
    pub name: Option<String>,
    /// The channel description.
    pub about: Option<String>,
    /// A URL for the channel picture.
    pub picture: Option<String>,
    /// Relays the channel's events live on.
    pub relays: Vec<String>,
}

/// Parse channel metadata JSON.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for non-object content or a
/// field of the wrong type.
pub fn channel_metadata(content: &str) -> Result<ChannelMetadata, DomainError> {
    let value: Value =
        serde_json::from_str(content).map_err(|_| invalid("channel metadata is a JSON object"))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("channel metadata is a JSON object"))?;
    let text = |field: &str| -> Result<Option<String>, DomainError> {
        match object.get(field) {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(|text| Some(text.to_string()))
                .ok_or_else(|| invalid("channel metadata fields are strings")),
        }
    };
    let mut relays = Vec::new();
    if let Some(value) = object.get("relays") {
        for relay in value
            .as_array()
            .ok_or_else(|| invalid("channel relays are an array"))?
        {
            let relay = relay
                .as_str()
                .ok_or_else(|| invalid("channel relays are URLs"))?;
            if !is_relay(relay) {
                return Err(invalid("channel relays are ws:// or wss:// URLs"));
            }
            relays.push(relay.to_string());
        }
    }
    Ok(ChannelMetadata {
        name: text("name")?,
        about: text("about")?,
        picture: text("picture")?,
        relays,
    })
}

/// A kind `40` channel creation: the metadata and the author's
/// declared relay set.
#[derive(Clone, Debug)]
pub struct Channel {
    /// The parsed content metadata.
    pub metadata: ChannelMetadata,
    /// Category names the `t` tags carry.
    pub categories: Vec<String>,
}

/// Read a kind `40` channel creation event.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or malformed
/// metadata.
pub fn open_channel(event: &Event) -> Result<Channel, DomainError> {
    if event.kind != 40 {
        return Err(invalid("a channel is created as kind 40"));
    }
    Ok(Channel {
        metadata: channel_metadata(&event.content)?,
        categories: event.tag_values("t").map(str::to_string).collect(),
    })
}

/// Read a kind `41` metadata update: an `e` root tag naming the
/// channel's kind `40` id plus the new metadata.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, no `e` root,
/// or malformed metadata.
pub fn open_channel_metadata(event: &Event) -> Result<(String, ChannelMetadata), DomainError> {
    if event.kind != 41 {
        return Err(invalid("channel metadata is kind 41"));
    }
    let channel = root_id(
        event,
        "a metadata update names its channel in an e root tag",
    )?;
    Ok((channel, channel_metadata(&event.content)?))
}

/// Whether a kind `41` update is authoritative for a channel: only
/// the channel's own author may set its metadata.
#[must_use]
pub fn metadata_updates_channel(update: &Event, channel: &Event) -> bool {
    update.kind == 41 && channel.kind == 40 && update.pubkey == channel.pubkey
}

/// A kind `42` channel message: the channel it roots at and the
/// message it replies to, when it is a reply.
#[derive(Clone, Debug)]
pub struct ChannelMessage {
    /// The kind `40` channel id — the `e` `root` tag.
    pub channel: String,
    /// The kind `42` id this replies to — the `e` `reply` tag.
    pub reply_to: Option<String>,
    /// The text.
    pub content: String,
}

/// Read a kind `42` channel message: an `e` `root` tag naming the
/// channel and, for replies, an `e` `reply` tag naming the parent
/// message with `p` tags for its participants.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no `e`
/// root.
pub fn open_channel_message(event: &Event) -> Result<ChannelMessage, DomainError> {
    if event.kind != 42 {
        return Err(invalid("a channel message is kind 42"));
    }
    let channel = root_id(
        event,
        "a channel message names its channel in an e root tag",
    )?;
    let reply_to = event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("e") && tag.0.get(3).is_some_and(|marker| marker == "reply"))
        .and_then(|tag| tag.value())
        .map(str::to_string);
    Ok(ChannelMessage {
        channel,
        reply_to,
        content: event.content.clone(),
    })
}

/// The kind `42` event a kind `43` hide targets, and the reason when
/// the content carries one.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no `e` tag.
pub fn open_hide_message(event: &Event) -> Result<(String, Option<String>), DomainError> {
    if event.kind != 43 {
        return Err(invalid("a hide-message is kind 43"));
    }
    let target = event
        .tag_values("e")
        .next()
        .filter(|value| decode_lower_hex::<32>(value, "e").is_ok())
        .ok_or_else(|| invalid("a hide-message names the hidden event in an e tag"))?
        .to_string();
    Ok((target, reason(&event.content)?))
}

/// The pubkey a kind `44` mute targets, and the reason when the
/// content carries one.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or no `p` tag.
pub fn open_mute_user(event: &Event) -> Result<(String, Option<String>), DomainError> {
    if event.kind != 44 {
        return Err(invalid("a mute-user is kind 44"));
    }
    let target = event
        .tag_values("p")
        .next()
        .filter(|value| decode_lower_hex::<32>(value, "p").is_ok())
        .ok_or_else(|| invalid("a mute-user names the muted author in a p tag"))?
        .to_string();
    Ok((target, reason(&event.content)?))
}

/// The `e` tag carrying the `root` marker — the channel a kind `41`
/// or `42` event belongs to.
fn root_id(event: &Event, why: &str) -> Result<String, DomainError> {
    event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("e") && tag.0.get(3).is_some_and(|marker| marker == "root"))
        .and_then(|tag| tag.value())
        .filter(|value| decode_lower_hex::<32>(value, "e").is_ok())
        .map(str::to_string)
        .ok_or_else(|| invalid(why))
}

/// A `{"reason": "…"}` content, when present — anything else is fine
/// as free text or empty.
fn reason(content: &str) -> Result<Option<String>, DomainError> {
    if content.is_empty() {
        return Ok(None);
    }
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return Ok(None);
    };
    match value.get("reason") {
        None => Ok(None),
        Some(reason) => reason
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| invalid("a reason is a string")),
    }
}

fn is_relay(value: &str) -> bool {
    value.starts_with("ws://") || value.starts_with("wss://")
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn channels_open_update_and_take_messages() {
        let channel = sign(
            40,
            vec![Tag::new(vec!["t".into(), "general".into()])],
            r#"{"name": "Demo Channel", "about": "A test channel.",
                "picture": "https://placekitten.com/200/200",
                "relays": ["wss://nos.lol", "wss://nostr.mom"]}"#,
        );
        let opened = open_channel(&channel).unwrap();
        assert_eq!(opened.metadata.name.as_deref(), Some("Demo Channel"));
        assert_eq!(opened.metadata.relays.len(), 2);
        assert_eq!(opened.categories, vec!["general"]);

        let update = sign(
            41,
            vec![Tag::new(vec![
                "e".into(),
                channel.id.clone(),
                "wss://r.example".into(),
                "root".into(),
            ])],
            r#"{"name": "Renamed"}"#,
        );
        let (target, metadata) = open_channel_metadata(&update).unwrap();
        assert_eq!(target, channel.id);
        assert_eq!(metadata.name.as_deref(), Some("Renamed"));
        // Only the channel's author updates it — the fixture signer is the
        // same key, so this is authoritative; a different key would not be.
        assert!(metadata_updates_channel(&update, &channel));
        let other = {
            let signer = RelaySigner::from_secret_hex(&"99".repeat(32)).unwrap();
            signer.sign(
                1_700_000_000,
                41,
                vec![Tag::new(vec![
                    "e".into(),
                    channel.id.clone(),
                    "wss://r".into(),
                    "root".into(),
                ])],
                r#"{"name": "hijack"}"#.to_string(),
            )
        };
        assert!(!metadata_updates_channel(&other, &channel));

        let root_msg = sign(
            42,
            vec![Tag::new(vec![
                "e".into(),
                channel.id.clone(),
                "wss://r.example".into(),
                "root".into(),
            ])],
            "hello channel",
        );
        let message = open_channel_message(&root_msg).unwrap();
        assert_eq!(message.channel, channel.id);
        assert_eq!(message.reply_to, None);

        let reply_msg = sign(
            42,
            vec![
                Tag::new(vec![
                    "e".into(),
                    channel.id.clone(),
                    "wss://r.example".into(),
                    "root".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    root_msg.id.clone(),
                    "wss://r.example".into(),
                    "reply".into(),
                ]),
                Tag::new(vec!["p".into(), root_msg.pubkey.clone()]),
            ],
            "replying",
        );
        let reply = open_channel_message(&reply_msg).unwrap();
        assert_eq!(reply.reply_to.as_deref(), Some(root_msg.id.as_str()));
    }

    #[test]
    fn hides_and_mutes_name_their_targets() {
        let hidden = "ef".repeat(32);
        let hide = sign(
            43,
            vec![Tag::new(vec!["e".into(), hidden.clone()])],
            r#"{"reason": "spam"}"#,
        );
        let (target, why) = open_hide_message(&hide).unwrap();
        assert_eq!(target, hidden);
        assert_eq!(why.as_deref(), Some("spam"));

        let muted = "cd".repeat(32);
        let mute = sign(44, vec![Tag::new(vec!["p".into(), muted.clone()])], "");
        let (target, why) = open_mute_user(&mute).unwrap();
        assert_eq!(target, muted);
        assert_eq!(why, None);
    }

    #[test]
    fn malformed_channel_events_are_refused() {
        assert!(open_channel(&sign(40, Vec::new(), "not json")).is_err());
        assert!(open_channel(&sign(40, Vec::new(), r#"{"relays": ["https://x"]}"#)).is_err());
        assert!(open_channel_metadata(&sign(41, Vec::new(), "{}")).is_err());
        assert!(open_channel_message(&sign(42, Vec::new(), "x")).is_err());
        assert!(open_hide_message(&sign(43, Vec::new(), "")).is_err());
        assert!(open_mute_user(&sign(44, Vec::new(), "")).is_err());
    }
}
