//! NIP-18 reposts.
//!
//! Kind `6` reposts a kind `1` note. Kind `16` reposts any other kind.
//! The `e` tag names the event id and a relay that can serve it. The
//! content is the JSON of that event, or empty. A protected event is not
//! embedded. `reply` quotes use a `q` tag.
//!
//! An empty repost does not prove the target kind, because the relay
//! does not fetch it. `nostr:` mentions in content are not rewritten
//! into `q` tags. These kinds are not added to the NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const NOTE_REPOST: u16 = 6;
const GENERIC_REPOST: u16 = 16;

/// A kind `6` or kind `16` repost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repost {
    pub kind: u16,
    pub target_id: String,
    pub relay: String,
    pub author: Option<String>,
    pub embedded_kind: Option<u16>,
    pub address: Option<ReplacementAddress>,
    pub quotes: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "repost pubkey")
        .map_err(|_| invalid("a repost pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn one_tag<'a>(
    event: &'a Event,
    name: &str,
    reason: &'static str,
) -> Result<&'a super::Tag, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid(reason));
    }
    Ok(tags[0])
}

fn optional_tag<'a>(
    event: &'a Event,
    name: &str,
    reason: &'static str,
) -> Result<Option<&'a super::Tag>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    Ok(tags.first().copied())
}

fn target(event: &Event) -> Result<(String, String), DomainError> {
    let tag = one_tag(event, "e", "a repost has one e tag")?;
    let Some(id) = tag.value() else {
        return Err(invalid("a repost event id is 32 lowercase hex bytes"));
    };
    decode_lower_hex::<32>(id, "repost event")
        .map_err(|_| invalid("a repost event id is 32 lowercase hex bytes"))?;
    let Some(relay) = tag.as_slice().get(2).filter(|value| is_relay(value)) else {
        return Err(invalid("a repost e tag includes a ws:// or wss:// relay"));
    };
    Ok((id.to_owned(), relay.to_owned()))
}

fn participants(event: &Event) -> Result<Vec<String>, DomainError> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .map(|tag| {
            let Some(value) = tag.value() else {
                return Err(invalid("a repost pubkey is 32 lowercase hex bytes"));
            };
            pubkey(value)
        })
        .collect()
}

fn quotes(event: &Event) -> Result<Vec<String>, DomainError> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("q"))
        .map(|tag| {
            let Some(value) = tag.value() else {
                return Err(invalid("a repost quote is an event id or an address"));
            };
            if decode_lower_hex::<32>(value, "repost quote").is_err()
                && ReplacementAddress::from_str(value).is_err()
            {
                return Err(invalid("a repost quote is an event id or an address"));
            }
            if let Some(relay) = tag.as_slice().get(2)
                && !relay.is_empty()
                && !is_relay(relay)
            {
                return Err(invalid("a repost relay hint is ws:// or wss://"));
            }
            if let Some(author) = tag.as_slice().get(3)
                && !author.is_empty()
            {
                pubkey(author)?;
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn embedded(event: &Event, target_id: &str) -> Result<Option<Event>, DomainError> {
    if event.content.is_empty() {
        return Ok(None);
    }
    let inner: Event = serde_json::from_str(&event.content)
        .map_err(|_| invalid("a repost embeds the JSON of the event or is empty"))?;
    inner
        .validate_nip01_structure()
        .map_err(|_| invalid("a repost embeds the JSON of the event or is empty"))?;
    inner
        .validate_crypto()
        .map_err(|_| invalid("a repost embeds a signed event"))?;
    if inner.id != target_id {
        return Err(invalid("a repost e tag matches the embedded event id"));
    }
    if inner.is_protected() {
        return Err(invalid("a repost of a protected event has empty content"));
    }
    Ok(Some(inner))
}

/// Read a kind `6` or kind `16` repost.
///
/// # Errors
///
/// Returns a sentence when the target, the embedded event, or a quote is refused.
pub fn open_repost(event: &Event) -> Result<Repost, DomainError> {
    if !matches!(event.kind, NOTE_REPOST | GENERIC_REPOST) {
        return Err(invalid("a repost has kind 6 or kind 16"));
    }
    let (target_id, relay) = target(event)?;
    let people = participants(event)?;
    let inner = embedded(event, &target_id)?;
    if event.kind == NOTE_REPOST {
        if let Some(note) = &inner {
            if note.kind != 1 {
                return Err(invalid("a kind 6 repost embeds a kind 1 note"));
            }
            super::note::open_note(note)?;
            if !people.is_empty() && !people.iter().any(|person| person == &note.pubkey) {
                return Err(invalid("a repost p tag names the embedded author"));
            }
        }
        return Ok(Repost {
            kind: event.kind,
            target_id,
            relay,
            author: inner
                .as_ref()
                .map(|note| note.pubkey.clone())
                .or_else(|| people.first().cloned()),
            embedded_kind: inner.as_ref().map(|note| note.kind),
            address: None,
            quotes: quotes(event)?,
        });
    }
    let kind_tag = optional_tag(event, "k", "a kind 16 repost has one k tag")?;
    let declared = match kind_tag.and_then(|tag| tag.value()) {
        None => None,
        Some(value) => Some(
            value
                .parse::<u16>()
                .map_err(|_| invalid("a kind 16 k tag is an event kind"))?,
        ),
    };
    if declared == Some(1) {
        return Err(invalid("a kind 16 repost is not a kind 1 note"));
    }
    let address = match optional_tag(event, "a", "a kind 16 repost has one a tag")? {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a kind 16 a tag is an event address"));
            };
            Some(
                ReplacementAddress::from_str(value)
                    .map_err(|_| invalid("a kind 16 a tag is an event address"))?,
            )
        }
    };
    if let Some(inner) = &inner {
        if inner.kind == 1 {
            return Err(invalid("a kind 16 repost is not a kind 1 note"));
        }
        if let Some(declared) = declared
            && declared != inner.kind
        {
            return Err(invalid("a kind 16 k tag matches the embedded kind"));
        }
        if let Some(address) = &address
            && inner.replacement_address().as_ref() != Some(address)
        {
            return Err(invalid("a kind 16 a tag matches the embedded address"));
        }
        if !people.is_empty() && !people.iter().any(|person| person == &inner.pubkey) {
            return Err(invalid("a repost p tag names the embedded author"));
        }
    }
    Ok(Repost {
        kind: event.kind,
        target_id,
        relay,
        author: inner
            .as_ref()
            .map(|inner| inner.pubkey.clone())
            .or_else(|| people.first().cloned()),
        embedded_kind: inner.as_ref().map(|inner| inner.kind).or(declared),
        address,
        quotes: quotes(event)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_repost_embeds_the_note_and_a_generic_repost_names_its_kind() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/18.md"
        ))
        .unwrap();
        assert!(text.contains("kind 6"));
        assert!(text.contains("kind 16"));
        assert!(text.contains("stringified JSON"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "18.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "18.md")
        );

        let author = signer("a1");
        let booster = signer("b2");
        let note = author.sign(1_700_000_000, 1, Vec::new(), "worth reading".into());
        note.validate_structure().unwrap();
        let repost = booster.sign(
            1_700_000_100,
            NOTE_REPOST,
            vec![
                Tag::new(vec![
                    "e".into(),
                    note.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec![
                    "q".into(),
                    "cd".repeat(32),
                    "wss://quotes.example".into(),
                ]),
            ],
            serde_json::to_string(&note).unwrap(),
        );
        repost.validate_structure().unwrap();
        assert_eq!(repost.class(), EventClass::Regular);
        let opened = open_repost(&repost).unwrap();
        assert_eq!(opened.target_id, note.id);
        assert_eq!(opened.author.as_deref(), Some(author.pubkey()));
        assert_eq!(opened.embedded_kind, Some(1));
        assert_eq!(opened.relay, "wss://relay.example");
        assert_eq!(opened.quotes.len(), 1);
        let again = booster.sign(
            1_700_000_150,
            NOTE_REPOST,
            vec![Tag::new(vec![
                "e".into(),
                note.id.clone(),
                "wss://relay.example".into(),
            ])],
            String::new(),
        );
        assert!(matches!(
            compare_replacement(&repost, &again),
            Err(DomainError::NotReplaceable)
        ));
        let empty = open_repost(&again).unwrap();
        assert!(empty.embedded_kind.is_none());
        assert!(!again.embeds_protected_repost());

        let protected = author.sign(
            1_700_000_200,
            1,
            vec![Tag::new(vec!["-".into()])],
            "secret".into(),
        );
        let leaked = booster.sign(
            1_700_000_210,
            NOTE_REPOST,
            vec![Tag::new(vec![
                "e".into(),
                protected.id.clone(),
                "wss://relay.example".into(),
            ])],
            serde_json::to_string(&protected).unwrap(),
        );
        assert!(leaked.embeds_protected_repost());
        assert!(leaked.validate_structure().is_err());
        let bare = booster.sign(
            1_700_000_220,
            NOTE_REPOST,
            vec![Tag::new(vec!["e".into(), note.id.clone()])],
            String::new(),
        );
        assert!(bare.validate_structure().is_err());

        let article = author.sign(
            1_700_000_300,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "Hello".into(),
        );
        article.validate_structure().unwrap();
        let address = format!("30023:{}:post", author.pubkey());
        let generic = booster.sign(
            1_700_000_310,
            GENERIC_REPOST,
            vec![
                Tag::new(vec![
                    "e".into(),
                    article.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["k".into(), "30023".into()]),
                Tag::new(vec!["a".into(), address]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
            ],
            serde_json::to_string(&article).unwrap(),
        );
        generic.validate_structure().unwrap();
        let generic_open = open_repost(&generic).unwrap();
        assert_eq!(generic_open.embedded_kind, Some(30_023));
        assert_eq!(
            generic_open
                .address
                .as_ref()
                .map(|value| value.identifier.as_str()),
            Some("post")
        );
        let note_as_generic = booster.sign(
            1_700_000_320,
            GENERIC_REPOST,
            vec![
                Tag::new(vec![
                    "e".into(),
                    note.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["k".into(), "1".into()]),
            ],
            serde_json::to_string(&note).unwrap(),
        );
        assert!(note_as_generic.validate_structure().is_err());
    }
}
