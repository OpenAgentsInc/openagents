//! NIP-A4 public messages.
//!
//! Kind `24` is a signed plaintext message to one or more receivers,
//! designed for notification screens rather than feeds. `p` tags name
//! the receivers with an optional relay hint, `e` tags are forbidden
//! because there are no threads, a `q` tag may cite an event, and an
//! `expiration` tag is recommended because a message without a
//! chatroom stops making sense over time. Reactions and zaps aimed at
//! kind `24` carry a `k` tag of `24`, and a NIP-21 `nevent1` link that
//! renders a public message natively must name kind `24`.
//!
//! There is no privacy in this kind — it is a public reply without a
//! root. The relay stores it like any kind `1` note, and NIP-A4 is a
//! draft, so kind `24` is not added to the NIP-11 list.

use crate::nip19;

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const PUBLIC_MESSAGE_KIND: u16 = 24;

/// One receiver a kind `24` message names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageReceiver {
    /// The receiver's 32-byte hex pubkey.
    pub pubkey: String,
    /// The inbox relay hint, when the tag carries one.
    pub relay: Option<String>,
}

/// A kind `24` public message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicMessage {
    /// The receivers, in the order the `p` tags listed them.
    pub receivers: Vec<MessageReceiver>,
    /// The plaintext content.
    pub content: String,
    /// The `expiration` timestamp, when the message carries one.
    pub expiration: Option<u64>,
}

/// Read a kind `24` event into a public message.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, no `p` tag, a
/// receiver that is not a 32-byte hex key, a malformed relay hint, or
/// any `e` tag — the pinned text has no threads.
pub fn open_public_message(event: &Event) -> Result<PublicMessage, DomainError> {
    if event.kind != PUBLIC_MESSAGE_KIND {
        return Err(invalid("a public message is kind 24"));
    }
    if event.tags.iter().any(|tag| tag.name() == Some("e")) {
        return Err(invalid("kind 24 has no threads: an e tag is forbidden"));
    }
    let mut receivers = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("p")) {
        let values = &tag.0[1..];
        let Some(pubkey) = values.first() else {
            return Err(invalid("a p tag names a receiver"));
        };
        if decode_lower_hex::<32>(pubkey, "p").is_err() {
            return Err(invalid("a receiver is a 32-byte hex pubkey"));
        }
        let relay = match values.get(1) {
            Some(url) if valid_relay_url(url) => Some(url.clone()),
            Some(_) => return Err(invalid("a receiver relay hint is a ws:// or wss:// URL")),
            None => None,
        };
        receivers.push(MessageReceiver {
            pubkey: pubkey.clone(),
            relay,
        });
    }
    if receivers.is_empty() {
        return Err(invalid("kind 24 names at least one receiver"));
    }
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("q")) {
        let Some(cited) = tag.value() else {
            return Err(invalid("a q tag cites an event"));
        };
        let quoted = decode_lower_hex::<32>(cited, "q").is_ok() || cited.contains(':');
        if !quoted {
            return Err(invalid("a q tag cites an event id or address"));
        }
    }
    Ok(PublicMessage {
        receivers,
        content: event.content.clone(),
        expiration: event.expiration(),
    })
}

/// Whether a reaction or zap event targets kind `24`: the pinned text
/// requires a `k` tag of `24` on both.
#[must_use]
pub fn targets_public_message(event: &Event) -> bool {
    event.tag_values("k").any(|kind| kind == "24")
}

/// The kind a `nevent1` or `naddr1` link declares, when it declares one.
///
/// A link that renders a public message natively must name kind `24`;
/// the kind is TLV record `3`, a big-endian `u32`.
#[must_use]
pub fn link_kind(entity: &str) -> Option<u16> {
    let (hrp, bytes) = nip19::decode(entity).ok()?;
    if hrp != "nevent" && hrp != "naddr" {
        return None;
    }
    let mut rest = bytes.as_slice();
    while rest.len() >= 2 {
        let (kind, length) = (rest[0], usize::from(rest[1]));
        rest = &rest[2..];
        if rest.len() < length {
            return None;
        }
        if kind == 3 && length == 4 {
            let declared = u32::from_be_bytes(rest[..4].try_into().ok()?);
            return u16::try_from(declared).ok();
        }
        rest = &rest[length..];
    }
    None
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

fn valid_relay_url(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn a_public_message_names_its_receivers_and_has_no_thread() {
        let receiver_one = "ab".repeat(32);
        let receiver_two = "cd".repeat(32);
        let message = sign(
            PUBLIC_MESSAGE_KIND,
            vec![
                Tag::new(vec!["p".into(), receiver_one.clone()]),
                Tag::new(vec![
                    "p".into(),
                    receiver_two.clone(),
                    "wss://inbox.example".into(),
                ]),
                Tag::new(vec!["expiration".into(), "1700000100".into()]),
                Tag::new(vec!["q".into(), "ef".repeat(32)]),
            ],
            "a plaintext hello",
        );
        let opened = open_public_message(&message).unwrap();
        assert_eq!(opened.receivers.len(), 2);
        assert_eq!(opened.receivers[0].pubkey, receiver_one);
        assert_eq!(
            opened.receivers[1].relay.as_deref(),
            Some("wss://inbox.example")
        );
        assert_eq!(opened.content, "a plaintext hello");
        assert_eq!(opened.expiration, Some(1_700_000_100));
        assert_eq!(
            EventClass::from_kind(PUBLIC_MESSAGE_KIND),
            EventClass::Regular
        );

        // A reaction aimed at the message carries k=24.
        let reaction = sign(
            7,
            vec![
                Tag::new(vec!["e".into(), message.id.clone()]),
                Tag::new(vec!["p".into(), message.pubkey.clone()]),
                Tag::new(vec!["k".into(), "24".into()]),
            ],
            "+",
        );
        assert!(targets_public_message(&reaction));
        let wrong_kind = sign(7, vec![Tag::new(vec!["k".into(), "1".into()])], "+");
        assert!(!targets_public_message(&wrong_kind));
    }

    #[test]
    fn a_nevent_link_declares_kind_24() {
        // TLV: id record (0x00, 32 bytes) then kind record (0x03, u32).
        let mut tlv = vec![0x00, 32];
        tlv.extend_from_slice(&[0xab; 32]);
        tlv.extend_from_slice(&[0x03, 0x04, 0x00, 0x00, 0x00, 0x18]);
        let link = nip19::encode("nevent", &tlv).unwrap();
        assert_eq!(link_kind(&link), Some(24));

        let mut without_kind = vec![0x00, 32];
        without_kind.extend_from_slice(&[0xab; 32]);
        let bare = nip19::encode("nevent", &without_kind).unwrap();
        assert_eq!(link_kind(&bare), None);
        assert_eq!(link_kind("npub1qqqq"), None);
    }

    #[test]
    fn threads_and_malformed_receivers_are_refused() {
        let receiver = "ab".repeat(32);
        let threaded = sign(
            PUBLIC_MESSAGE_KIND,
            vec![
                Tag::new(vec!["p".into(), receiver.clone()]),
                Tag::new(vec!["e".into(), "cd".repeat(32)]),
            ],
            "a reply-shaped message",
        );
        assert!(open_public_message(&threaded).is_err());

        assert!(open_public_message(&sign(PUBLIC_MESSAGE_KIND, Vec::new(), "hi")).is_err());
        let bad_key = sign(
            PUBLIC_MESSAGE_KIND,
            vec![Tag::new(vec!["p".into(), "not hex".into()])],
            "hi",
        );
        assert!(open_public_message(&bad_key).is_err());
        let bad_relay = sign(
            PUBLIC_MESSAGE_KIND,
            vec![Tag::new(vec![
                "p".into(),
                receiver,
                "http://not-a-relay".into(),
            ])],
            "hi",
        );
        assert!(open_public_message(&bad_relay).is_err());
        assert!(open_public_message(&sign(1, Vec::new(), "note")).is_err());
    }
}
