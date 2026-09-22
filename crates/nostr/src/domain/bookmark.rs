//! NIP-B0 web bookmarks.
//!
//! Kind `39701` is an addressable bookmark. The `d` tag is the URI. When
//! the scheme is `https`, the characters before the hostname are omitted,
//! so `https://alice.blog/post` is stored as `alice.blog/post`. `http`
//! keeps its scheme. The content is a description and may be empty.
//!
//! A reply is a kind `1111` comment, not a kind `1` note. The relay does
//! not fetch the page. Kind `39701` is not added to the NIP-11 list.

use super::{DomainError, Event};

const KIND: u16 = 39_701;

/// A web bookmark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub identifier: String,
    pub content: String,
    pub title: Option<String>,
    pub published_at: Option<u64>,
    pub topics: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn strip_userinfo(rest: &str) -> &str {
    match rest.split_once('@') {
        Some((before, after)) if !before.contains('/') => after,
        _ => rest,
    }
}

/// The `d` tag for `uri`. An `https` URI loses the scheme and any userinfo.
///
/// # Errors
///
/// Returns a sentence when `uri` is empty, contains whitespace, or has no host.
pub fn bookmark_identifier(uri: &str) -> Result<String, DomainError> {
    if uri.is_empty() || uri.len() > 2_048 || uri.chars().any(char::is_whitespace) {
        return Err(invalid(
            "a bookmark URI contains 1 to 2048 characters and no whitespace",
        ));
    }
    if let Some(rest) = uri.strip_prefix("https://") {
        let host = strip_userinfo(rest);
        if host.is_empty() || host.starts_with('/') {
            return Err(invalid("a bookmark URI names a host"));
        }
        return Ok(host.to_owned());
    }
    if let Some(rest) = uri.strip_prefix("http://") {
        let host = strip_userinfo(rest);
        if host.is_empty() || host.starts_with('/') {
            return Err(invalid("a bookmark URI names a host"));
        }
        return Ok(uri.to_owned());
    }
    if uri.contains("://") || uri.starts_with('/') {
        return Err(invalid(
            "a bookmark d tag omits the https scheme and keeps other schemes intact",
        ));
    }
    Ok(uri.to_owned())
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<Option<&'a str>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    match tags.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

/// Read a kind `39701` bookmark.
///
/// # Errors
///
/// Returns a sentence when the kind or the `d` tag is refused.
pub fn open_bookmark(event: &Event) -> Result<Bookmark, DomainError> {
    if event.kind != KIND {
        return Err(invalid("a web bookmark has kind 39701"));
    }
    let identifiers = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect::<Vec<_>>();
    if identifiers.len() != 1 {
        return Err(invalid("a web bookmark has one d tag"));
    }
    let Some(identifier) = identifiers[0].value() else {
        return Err(invalid("a web bookmark has one d tag"));
    };
    let normalized = bookmark_identifier(identifier)?;
    if normalized != identifier {
        return Err(invalid(
            "a bookmark d tag omits the characters before the hostname when the scheme is https",
        ));
    }
    let published_at = match one(
        event,
        "published_at",
        "a bookmark published_at is one unix timestamp",
    )? {
        None => None,
        Some(value) => {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| invalid("a bookmark published_at is one unix timestamp"))?;
            if parsed.to_string() != value {
                return Err(invalid("a bookmark published_at is one unix timestamp"));
            }
            Some(parsed)
        }
    };
    let mut topics = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("t")) {
        let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
            return Err(invalid("a bookmark topic is non-empty"));
        };
        topics.push(value.to_owned());
    }
    Ok(Bookmark {
        identifier: identifier.to_owned(),
        content: event.content.clone(),
        title: one(event, "title", "a bookmark title is one non-empty value")?.map(str::to_owned),
        published_at,
        topics,
    })
}

/// Whether `event` is a kind `1111` comment on a kind `39701` bookmark.
pub fn is_bookmark_reply(event: &Event) -> bool {
    event.kind == 1_111
        && super::comment::open_comment(event).is_ok()
        && event
            .tag_values("K")
            .chain(event.tag_values("k"))
            .any(|kind| kind == "39701")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EventClass, Filter, RelaySigner, ReplacementDecision, Tag, compare_replacement,
        is_top_level, open_comment,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"b0".repeat(32)).unwrap()
    }

    #[test]
    fn a_bookmark_drops_the_https_scheme_and_a_comment_replies() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/B0.md"
        ))
        .unwrap();
        assert!(text.contains("kind:39701"));
        assert!(text.contains("alice.blog/post"));
        assert!(text.contains("kind 1111"));
        assert!(text.contains("d7a92714f81d0f712e715556aee69ea6da6bfb287e6baf794a095d301d603ec7"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "B0.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "B0.md")
        );

        assert_eq!(
            bookmark_identifier("https://alice.blog/post").unwrap(),
            "alice.blog/post"
        );
        assert_eq!(
            bookmark_identifier("https://alice:secret@alice.blog/post?x=1").unwrap(),
            "alice.blog/post?x=1"
        );
        assert_eq!(
            bookmark_identifier("http://alice.blog/post").unwrap(),
            "http://alice.blog/post"
        );

        let identifier = bookmark_identifier("https://alice.blog/post").unwrap();
        let event = signer().sign(
            1_738_869_705,
            KIND,
            vec![
                Tag::new(vec!["d".into(), identifier.clone()]),
                Tag::new(vec!["published_at".into(), "1738863000".into()]),
                Tag::new(vec!["title".into(), "Blog insights by Alice".into()]),
                Tag::new(vec!["t".into(), "post".into()]),
                Tag::new(vec!["t".into(), "insight".into()]),
            ],
            "A marvelous insight by Alice about the nature of blogs and posts.".into(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Addressable);
        let bookmark = open_bookmark(&event).unwrap();
        assert_eq!(bookmark.identifier, "alice.blog/post");
        assert_eq!(bookmark.title.as_deref(), Some("Blog insights by Alice"));
        assert_eq!(bookmark.published_at, Some(1_738_863_000));
        assert_eq!(
            bookmark.topics,
            vec!["post".to_owned(), "insight".to_owned()]
        );
        assert!(bookmark.content.contains("marvelous insight"));

        let newer = signer().sign(
            1_738_869_800,
            KIND,
            vec![
                Tag::new(vec!["d".into(), identifier.clone()]),
                Tag::new(vec!["title".into(), "Blog insights by Alice".into()]),
            ],
            String::new(),
        );
        newer.validate_structure().unwrap();
        assert!(open_bookmark(&newer).unwrap().content.is_empty());
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let mut filter = Filter {
            kinds: Some(vec![KIND]),
            authors: Some(vec![signer().pubkey().to_owned()]),
            ..Filter::default()
        };
        filter.tags.insert("d".into(), vec![identifier.clone()]);
        assert!(filter.matches(&event));

        let address = format!("39701:{}:{identifier}", signer().pubkey());
        let reply = signer().sign(
            1_738_869_900,
            1_111,
            vec![
                Tag::new(vec![
                    "A".into(),
                    address.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["K".into(), "39701".into()]),
                Tag::new(vec![
                    "P".into(),
                    signer().pubkey().to_owned(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["a".into(), address, "wss://relay.example".into()]),
                Tag::new(vec![
                    "e".into(),
                    event.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["k".into(), "39701".into()]),
                Tag::new(vec![
                    "p".into(),
                    signer().pubkey().to_owned(),
                    "wss://relay.example".into(),
                ]),
            ],
            "Useful bookmark.".into(),
        );
        reply.validate_structure().unwrap();
        assert!(is_bookmark_reply(&reply));
        assert!(is_top_level(&open_comment(&reply).unwrap()));
        let note = signer().sign(1_738_869_901, 1, Vec::new(), "not a reply".into());
        assert!(!is_bookmark_reply(&note));

        let prefixed = signer().sign(
            1_738_869_902,
            KIND,
            vec![Tag::new(vec!["d".into(), "https://alice.blog/post".into()])],
            String::new(),
        );
        assert!(prefixed.validate_structure().is_err());
    }
}
