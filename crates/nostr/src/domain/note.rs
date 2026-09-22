//! NIP-10 text notes and threads.
//!
//! Kind `1` is a regular plaintext note. A marked `e` tag of `root` names
//! the thread root, and `reply` names the direct parent. A direct reply to
//! the root carries only the `root` marker. Unmarked `e` tags are the
//! deprecated positional form: one tag is a direct reply, and two or more
//! put the root first and the parent last. Middle tags are mentions.
//! A `q` tag cites an event id or an address. `reply_participants` places
//! the replied-to author first and then the pubkeys already involved.
//!
//! The relay does not fetch the referenced event, so it does not prove
//! that an `e` tag points at kind `1`. Markup in the content is kept.
//! Kind `1` threading is not added to the NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const NOTE_KIND: u16 = 1;

/// One `e` tag on a kind `1` note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRef {
    pub id: String,
    pub relay: String,
    pub author: Option<String>,
}

/// One `q` tag citing an event id or a replacement address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteQuote {
    pub target: String,
    pub relay: String,
    pub author: Option<String>,
}

/// A kind `1` text note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextNote {
    pub content: String,
    pub root: Option<NoteRef>,
    pub parent: Option<NoteRef>,
    pub mentions: Vec<NoteRef>,
    pub quotes: Vec<NoteQuote>,
    pub participants: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "note event")
        .map_err(|_| invalid("a note event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "note pubkey")
        .map_err(|_| invalid("a note pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn relay_hint(value: &str) -> Result<String, DomainError> {
    if value.is_empty() || is_relay(value) {
        Ok(value.to_owned())
    } else {
        Err(invalid("a note relay hint is empty or ws:// or wss://"))
    }
}

fn optional_author(value: Option<&String>) -> Result<Option<String>, DomainError> {
    match value {
        None => Ok(None),
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => Ok(Some(pubkey(value)?)),
    }
}

fn note_ref(tag: &super::Tag) -> Result<NoteRef, DomainError> {
    let Some(id) = tag.value() else {
        return Err(invalid("a note event id is 32 lowercase hex bytes"));
    };
    let relay = match tag.as_slice().get(2) {
        None => String::new(),
        Some(value) => relay_hint(value)?,
    };
    Ok(NoteRef {
        id: event_id(id)?,
        relay,
        author: optional_author(tag.as_slice().get(4))?,
    })
}

/// The `p` tags for a reply to `author`, then each pubkey in `involved`.
///
/// The author is first. A later copy of a pubkey already listed is omitted.
///
/// # Errors
///
/// Returns a sentence when a value is not 32 lowercase hex bytes.
pub fn reply_participants(author: &str, involved: &[String]) -> Result<Vec<String>, DomainError> {
    let mut participants = vec![pubkey(author)?];
    for person in involved {
        let person = pubkey(person)?;
        if !participants.iter().any(|seen| seen == &person) {
            participants.push(person);
        }
    }
    Ok(participants)
}

/// True when the note replies directly to the thread root.
#[must_use]
pub fn is_direct_reply(note: &TextNote) -> bool {
    match (&note.root, &note.parent) {
        (Some(root), Some(parent)) => root.id == parent.id,
        _ => false,
    }
}

struct Thread {
    root: Option<NoteRef>,
    parent: Option<NoteRef>,
    mentions: Vec<NoteRef>,
}

fn thread(tags: &[super::Tag]) -> Result<Thread, DomainError> {
    let mut marked = false;
    let mut positional = Vec::new();
    let mut root = None;
    let mut reply = None;
    let mut mentions = Vec::new();
    for tag in tags.iter().filter(|tag| tag.name() == Some("e")) {
        let reference = note_ref(tag)?;
        match tag.as_slice().get(3).map(String::as_str) {
            Some("root") => {
                marked = true;
                if root.replace(reference).is_some() {
                    return Err(invalid("a note has one root"));
                }
            }
            Some("reply") => {
                marked = true;
                if reply.replace(reference).is_some() {
                    return Err(invalid("a note has one reply parent"));
                }
            }
            Some(_) => return Err(invalid("a note marker is root or reply")),
            None => positional.push(reference),
        }
    }
    if marked {
        let parent = reply.or_else(|| root.clone());
        Ok(Thread {
            root,
            parent,
            mentions: positional,
        })
    } else {
        match positional.len() {
            0 => Ok(Thread {
                root: None,
                parent: None,
                mentions,
            }),
            1 => {
                let only = positional.remove(0);
                Ok(Thread {
                    root: Some(only.clone()),
                    parent: Some(only),
                    mentions,
                })
            }
            _ => {
                let parent = positional.remove(positional.len() - 1);
                let root = positional.remove(0);
                mentions = positional;
                Ok(Thread {
                    root: Some(root),
                    parent: Some(parent),
                    mentions,
                })
            }
        }
    }
}

fn quote(tag: &super::Tag) -> Result<NoteQuote, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid("a note quote is an event id or an address"));
    };
    if event_id(value).is_err() && ReplacementAddress::from_str(value).is_err() {
        return Err(invalid("a note quote is an event id or an address"));
    }
    let relay = match tag.as_slice().get(2) {
        None => String::new(),
        Some(hint) => relay_hint(hint)?,
    };
    Ok(NoteQuote {
        target: value.to_owned(),
        relay,
        author: optional_author(tag.as_slice().get(3))?,
    })
}

/// Read a kind `1` text note.
///
/// # Errors
///
/// Returns a sentence when an event id, marker, relay hint, quote, or
/// participant pubkey is refused.
pub fn open_note(event: &Event) -> Result<TextNote, DomainError> {
    if event.kind != NOTE_KIND {
        return Err(invalid("a text note has kind 1"));
    }
    let Thread {
        root,
        parent,
        mentions,
    } = thread(&event.tags)?;
    let mut quotes = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("q")) {
        quotes.push(quote(tag)?);
    }
    let mut participants = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("p")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a note pubkey is 32 lowercase hex bytes"));
        };
        participants.push(pubkey(value)?);
        if let Some(hint) = tag.as_slice().get(2)
            && !hint.is_empty()
        {
            relay_hint(hint)?;
        }
    }
    Ok(TextNote {
        content: event.content.clone(),
        root,
        parent,
        mentions,
        quotes,
        participants,
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
    fn a_note_threads_from_the_root_to_its_parent() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/10.md"
        ))
        .unwrap();
        assert!(text.contains("kind:1"));
        assert!(text.contains("\"root\""));
        assert!(text.contains("\"reply\""));
        assert!(text.contains("Deprecated Positional"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "10.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "10.md")
        );

        let author = signer("a1");
        let replier = signer("b2");
        let note = author.sign(
            1_700_000_000,
            NOTE_KIND,
            Vec::new(),
            "**hello** from the root".into(),
        );
        note.validate_structure().unwrap();
        assert_eq!(note.class(), EventClass::Regular);
        let opened = open_note(&note).unwrap();
        assert_eq!(opened.content, "**hello** from the root");
        assert!(opened.root.is_none());
        assert!(opened.parent.is_none());
        assert!(!is_direct_reply(&opened));
        let later = author.sign(1_700_000_050, NOTE_KIND, Vec::new(), "again".into());
        assert!(matches!(
            compare_replacement(&note, &later),
            Err(DomainError::NotReplaceable)
        ));

        let p1 = "11".repeat(32);
        let p2 = "22".repeat(32);
        let p3 = "33".repeat(32);
        let involved = vec![
            p1.clone(),
            p2.clone(),
            p3.clone(),
            author.pubkey().to_owned(),
        ];
        assert_eq!(
            reply_participants(author.pubkey(), &involved).unwrap(),
            vec![
                author.pubkey().to_owned(),
                p1.clone(),
                p2.clone(),
                p3.clone()
            ]
        );
        assert!(reply_participants("zz", &involved).is_err());

        let direct = replier.sign(
            1_700_000_100,
            NOTE_KIND,
            vec![
                Tag::new(vec![
                    "e".into(),
                    note.id.clone(),
                    "wss://relay.example".into(),
                    "root".into(),
                    author.pubkey().to_owned(),
                ]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec!["p".into(), p1.clone()]),
            ],
            "direct reply".into(),
        );
        direct.validate_structure().unwrap();
        let direct_note = open_note(&direct).unwrap();
        assert!(is_direct_reply(&direct_note));
        assert_eq!(direct_note.root.as_ref().unwrap().id, note.id);
        assert_eq!(
            direct_note.root.as_ref().unwrap().author.as_deref(),
            Some(author.pubkey())
        );
        assert_eq!(direct_note.participants, vec![author.pubkey(), p1.as_str()]);

        let mention = "cd".repeat(32);
        let quoted = "ee".repeat(32);
        let nested = author.sign(
            1_700_000_200,
            NOTE_KIND,
            vec![
                Tag::new(vec![
                    "e".into(),
                    note.id.clone(),
                    String::new(),
                    "root".into(),
                    author.pubkey().to_owned(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    direct.id.clone(),
                    "wss://relay.example".into(),
                    "reply".into(),
                    replier.pubkey().to_owned(),
                ]),
                Tag::new(vec!["e".into(), mention.clone(), String::new()]),
                Tag::new(vec![
                    "q".into(),
                    quoted.clone(),
                    "wss://quotes.example".into(),
                    replier.pubkey().to_owned(),
                ]),
                Tag::new(vec!["q".into(), format!("30023:{}:hello", author.pubkey())]),
                Tag::new(vec!["p".into(), replier.pubkey().to_owned()]),
            ],
            "nested reply".into(),
        );
        nested.validate_structure().unwrap();
        let nested_note = open_note(&nested).unwrap();
        assert!(!is_direct_reply(&nested_note));
        assert_eq!(nested_note.root.as_ref().unwrap().id, note.id);
        assert_eq!(nested_note.parent.as_ref().unwrap().id, direct.id);
        assert_eq!(nested_note.mentions.len(), 1);
        assert_eq!(nested_note.mentions[0].id, mention);
        assert_eq!(nested_note.quotes.len(), 2);
        assert_eq!(nested_note.quotes[0].target, quoted);
        assert!(nested_note.quotes[1].target.contains(":hello"));

        let positional = replier.sign(
            1_700_000_300,
            NOTE_KIND,
            vec![
                Tag::new(vec!["e".into(), note.id.clone(), String::new()]),
                Tag::new(vec!["e".into(), mention.clone()]),
                Tag::new(vec!["e".into(), direct.id.clone()]),
            ],
            "positional reply".into(),
        );
        positional.validate_structure().unwrap();
        let old = open_note(&positional).unwrap();
        assert_eq!(old.root.as_ref().unwrap().id, note.id);
        assert_eq!(old.parent.as_ref().unwrap().id, direct.id);
        assert_eq!(old.mentions[0].id, mention);

        let one = replier.sign(
            1_700_000_400,
            NOTE_KIND,
            vec![Tag::new(vec!["e".into(), note.id.clone()])],
            "one positional reply".into(),
        );
        let one = open_note(&one).unwrap();
        assert!(is_direct_reply(&one));
        assert_eq!(one.parent.as_ref().unwrap().id, note.id);

        let marked_wrong = replier.sign(
            1_700_000_500,
            NOTE_KIND,
            vec![Tag::new(vec![
                "e".into(),
                note.id.clone(),
                String::new(),
                "mention".into(),
            ])],
            "bad marker".into(),
        );
        assert!(marked_wrong.validate_structure().is_err());
        let bad_relay = replier.sign(
            1_700_000_600,
            NOTE_KIND,
            vec![Tag::new(vec![
                "e".into(),
                note.id,
                "https://example.com".into(),
                "root".into(),
            ])],
            "bad relay".into(),
        );
        assert!(bad_relay.validate_structure().is_err());
    }
}
