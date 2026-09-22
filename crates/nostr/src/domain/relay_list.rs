//! NIP-65 relay list metadata.
//!
//! Kind `10002` is a replaceable list of `r` tags. A missing marker means
//! the relay is both read and write. `read` and `write` narrow it. Clients
//! publish an event to the author's write relays and to each tagged user's
//! read relays.
//!
//! The relay stores the newest list. It does not choose where a client
//! publishes, and it does not enforce the 2-4 relay guidance.

use super::{DomainError, Event};

const KIND: u16 = 10_002;

/// Whether a relay accepts reads, writes, or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayMarker {
    Read,
    Write,
    Both,
}

/// One relay in a kind `10002` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedRelay {
    pub url: String,
    pub marker: RelayMarker,
}

/// A user's relay list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayList {
    pub relays: Vec<ListedRelay>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

/// Read a kind `10002` list.
///
/// # Errors
///
/// Returns a sentence when an `r` tag is missing or its marker is not
/// `read` or `write`.
pub fn open_relay_list(event: &Event) -> Result<RelayList, DomainError> {
    if event.kind != KIND {
        return Err(invalid("a relay list has kind 10002"));
    }
    let mut relays = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("r")) {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 3 || !is_relay(&values[1]) {
            return Err(invalid(
                "kind 10002 requires valid r relay tags and read/write markers",
            ));
        }
        let marker = match values.get(2).map(String::as_str) {
            None => RelayMarker::Both,
            Some("read") => RelayMarker::Read,
            Some("write") => RelayMarker::Write,
            Some(_) => {
                return Err(invalid(
                    "kind 10002 requires valid r relay tags and read/write markers",
                ));
            }
        };
        relays.push(ListedRelay {
            url: values[1].clone(),
            marker,
        });
    }
    if relays.is_empty() {
        return Err(invalid(
            "kind 10002 requires valid r relay tags and read/write markers",
        ));
    }
    Ok(RelayList { relays })
}

/// Relays the user writes to, in list order.
pub fn write_relays(list: &RelayList) -> Vec<&str> {
    list.relays
        .iter()
        .filter(|relay| matches!(relay.marker, RelayMarker::Write | RelayMarker::Both))
        .map(|relay| relay.url.as_str())
        .collect()
}

/// Relays where the user reads mentions, in list order.
pub fn read_relays(list: &RelayList) -> Vec<&str> {
    list.relays
        .iter()
        .filter(|relay| matches!(relay.marker, RelayMarker::Read | RelayMarker::Both))
        .map(|relay| relay.url.as_str())
        .collect()
}

/// Author write relays, then each mentioned user's read relays, without repeats.
pub fn publish_relays(author: &RelayList, mentioned: &[&RelayList]) -> Vec<String> {
    let mut urls = Vec::new();
    for url in write_relays(author) {
        push_unique(&mut urls, url);
    }
    for list in mentioned {
        for url in read_relays(list) {
            push_unique(&mut urls, url);
        }
    }
    urls
}

fn push_unique(urls: &mut Vec<String>, url: &str) {
    if !urls.iter().any(|existing| existing == url) {
        urls.push(url.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"65".repeat(32)).unwrap()
    }

    fn list(created_at: u64, tags: Vec<Tag>) -> Event {
        signer().sign(created_at, KIND, tags, String::new())
    }

    #[test]
    fn a_relay_list_splits_read_and_write_and_replaces() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/65.md"
        ))
        .unwrap();
        assert!(text.contains("kind:10002"));
        assert!(text.contains("wss://alicerelay.example.com"));
        assert!(text.contains("\"write\""));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "65.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "65.md")
        );

        let event = list(
            1_700_000_000,
            vec![
                Tag::new(vec!["r".into(), "wss://alicerelay.example.com".into()]),
                Tag::new(vec!["r".into(), "wss://brando-relay.com".into()]),
                Tag::new(vec![
                    "r".into(),
                    "wss://expensive-relay.example2.com".into(),
                    "write".into(),
                ]),
                Tag::new(vec![
                    "r".into(),
                    "wss://nostr-relay.example.com".into(),
                    "read".into(),
                ]),
            ],
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Replaceable);
        let opened = open_relay_list(&event).unwrap();
        assert_eq!(
            write_relays(&opened),
            vec![
                "wss://alicerelay.example.com",
                "wss://brando-relay.com",
                "wss://expensive-relay.example2.com",
            ]
        );
        assert_eq!(
            read_relays(&opened),
            vec![
                "wss://alicerelay.example.com",
                "wss://brando-relay.com",
                "wss://nostr-relay.example.com",
            ]
        );

        let mentioned = list(
            1_700_000_000,
            vec![Tag::new(vec![
                "r".into(),
                "wss://mentions.example".into(),
                "read".into(),
            ])],
        );
        let mentioned = open_relay_list(&mentioned).unwrap();
        assert_eq!(
            publish_relays(&opened, &[&mentioned]),
            vec![
                "wss://alicerelay.example.com".to_owned(),
                "wss://brando-relay.com".to_owned(),
                "wss://expensive-relay.example2.com".to_owned(),
                "wss://mentions.example".to_owned(),
            ]
        );

        let newer = list(
            1_700_000_100,
            vec![Tag::new(vec![
                "r".into(),
                "wss://alicerelay.example.com".into(),
            ])],
        );
        newer.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let marked = list(
            1_700_000_200,
            vec![Tag::new(vec![
                "r".into(),
                "wss://alicerelay.example.com".into(),
                "both".into(),
            ])],
        );
        assert!(marked.validate_structure().is_err());
        let empty = list(1_700_000_300, Vec::new());
        assert!(empty.validate_structure().is_err());
    }
}
