//! NIP-C7 chats.
//!
//! A chat message is a kind `9` event with plaintext content. A reply
//! is another kind `9` that quotes its parent in a `q` tag — the
//! NIP-18 quote form of an event id or address, an optional relay URL,
//! and an optional author pubkey. A chat view fetches kind `9` only,
//! which is what [`chat_filter`] returns; other content types may sit
//! inside the content as NIP-18 quotes without joining the stream.
//!
//! The relay stores kind `9` like any regular event, and NIP-C7 is a
//! draft, so it is not added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Filter};

const CHAT_KIND: u16 = 9;

/// A kind `9` chat message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    /// The plaintext content.
    pub content: String,
    /// The quoted parents — replies carry at least one.
    pub quotes: Vec<ChatQuote>,
}

/// A `q` tag on a chat message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatQuote {
    /// The event id or address the message quotes.
    pub target: String,
    /// The relay hint, when the tag carries one.
    pub relay: Option<String>,
    /// The quoted author's pubkey, when the tag carries one.
    pub author: Option<String>,
}

/// The filter a chat view fetches: kind `9` only, so no implementation
/// misses another's context.
#[must_use]
pub fn chat_filter() -> Filter {
    Filter {
        kinds: Some(vec![CHAT_KIND]),
        ..Filter::default()
    }
}

/// Read a kind `9` event into a chat message.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or a malformed
/// `q` tag — the target must be an event id or address, the relay hint
/// a URL, and the author a 32-byte hex key.
pub fn open_chat(event: &Event) -> Result<Chat, DomainError> {
    if event.kind != CHAT_KIND {
        return Err(invalid("a chat message is kind 9"));
    }
    let mut quotes = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("q")) {
        let values = &tag.0[1..];
        let Some(target) = values.first() else {
            return Err(invalid("a q tag quotes an event"));
        };
        if decode_lower_hex::<32>(target, "q").is_err() && !target.contains(':') {
            return Err(invalid("a q tag quotes an event id or address"));
        }
        let relay = match values.get(1) {
            Some(url) if valid_url(url) => Some(url.clone()),
            Some(_) => return Err(invalid("a quote relay hint is a URL")),
            None => None,
        };
        let author = match values.get(2) {
            Some(pubkey) if decode_lower_hex::<32>(pubkey, "q author").is_ok() => {
                Some(pubkey.clone())
            }
            Some(_) => return Err(invalid("a quote author is a 32-byte hex key")),
            None => None,
        };
        quotes.push(ChatQuote {
            target: target.clone(),
            relay,
            author,
        });
    }
    Ok(Chat {
        content: event.content.clone(),
        quotes,
    })
}

/// Whether the message replies to a parent: one or more `q` tags.
#[must_use]
pub fn is_chat_reply(event: &Event) -> bool {
    event.kind == CHAT_KIND && event.tags.iter().any(|tag| tag.name() == Some("q"))
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

fn valid_url(value: &str) -> bool {
    (value.starts_with("ws://")
        || value.starts_with("wss://")
        || value.starts_with("http://")
        || value.starts_with("https://"))
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
    fn a_chat_reply_quotes_its_parent_and_the_stream_fetches_kind_9() {
        let parent = sign(CHAT_KIND, Vec::new(), "GM");
        let reply = sign(
            CHAT_KIND,
            vec![Tag::new(vec![
                "q".into(),
                parent.id.clone(),
                "wss://relay.example".into(),
                parent.pubkey.clone(),
            ])],
            "nostr:nevent1...\nyes",
        );
        let chat = open_chat(&reply).unwrap();
        assert!(is_chat_reply(&reply));
        assert!(!is_chat_reply(&parent));
        assert_eq!(chat.quotes.len(), 1);
        assert_eq!(chat.quotes[0].target, parent.id);
        assert_eq!(
            chat.quotes[0].author.as_deref(),
            Some(parent.pubkey.as_str())
        );

        let filter = chat_filter();
        assert_eq!(filter.kinds, Some(vec![9]));
        assert!(crate::domain::matches_any(
            std::slice::from_ref(&filter),
            &reply
        ));
        let note = sign(1, Vec::new(), "not a chat");
        assert!(!crate::domain::matches_any(&[filter], &note));
        assert_eq!(EventClass::from_kind(CHAT_KIND), EventClass::Regular);
    }

    #[test]
    fn malformed_quotes_are_refused() {
        assert!(open_chat(&sign(1, Vec::new(), "note")).is_err());
        let empty_quote = sign(CHAT_KIND, vec![Tag::new(vec!["q".into()])], "hi");
        assert!(open_chat(&empty_quote).is_err());
        let bad_target = sign(
            CHAT_KIND,
            vec![Tag::new(vec!["q".into(), "not an id".into()])],
            "hi",
        );
        assert!(open_chat(&bad_target).is_err());
        let bad_author = sign(
            CHAT_KIND,
            vec![Tag::new(vec![
                "q".into(),
                "ab".repeat(32),
                "wss://relay.example".into(),
                "not hex".into(),
            ])],
            "hi",
        );
        assert!(open_chat(&bad_author).is_err());
    }
}
