//! NIP-22 comments.
//!
//! A kind `1111` comment is a regular event. Uppercase tags name the root
//! scope and lowercase tags name the parent. `K` and `k` are required.
//! Kind `1` can be a root or a parent. External scopes use the NIP-73
//! identifier types.
//!
//! Content is kept as text. This module does not render it and does not
//! strip markup. A URL is refused when it contains a fragment; the crate
//! does not rewrite the URL into a normalized form.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const COMMENT_KIND: u16 = 1_111;

/// Where a comment is rooted or parented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentScope {
    /// An `A` or `a` tag.
    Address {
        address: ReplacementAddress,
        relay: Option<String>,
        author: String,
    },
    /// An `E` or `e` tag.
    Event {
        id: String,
        relay: Option<String>,
        author: String,
    },
    /// An `I` or `i` tag.
    External {
        value: String,
        kind: String,
        hint: Option<String>,
    },
}

/// A validated kind `1111` comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub content: String,
    pub root: CommentScope,
    pub parent: CommentScope,
    /// Event id of an addressable or replaceable parent.
    pub parent_version: Option<String>,
    pub quotes: Vec<String>,
    pub mentions: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn is_http(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.contains('#')
        && !value.chars().any(char::is_whitespace)
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "comment author")
        .map_err(|_| invalid("a comment author must be a pubkey"))?;
    Ok(value.to_owned())
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "comment event id")
        .map_err(|_| invalid("a comment event id must be 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn hint(tag: &super::Tag, index: usize, external: bool) -> Result<Option<String>, DomainError> {
    let Some(value) = tag.as_slice().get(index) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err(invalid("a comment hint is empty"));
    }
    let ok = if external {
        is_http(value) || is_relay(value)
    } else {
        is_relay(value)
    };
    if ok {
        Ok(Some(value.to_owned()))
    } else {
        Err(invalid("a comment hint is not a relay or URL"))
    }
}

fn nostr_kind(value: &str) -> Result<u16, DomainError> {
    let kind = value
        .parse::<u16>()
        .map_err(|_| invalid("a comment kind must be a number or an external type"))?;
    Ok(kind)
}

fn one<'a>(tags: &'a [super::Tag], name: &str) -> Result<Option<&'a super::Tag>, DomainError> {
    let mut found = tags.iter().filter(|tag| tag.name() == Some(name));
    let tag = found.next();
    if found.next().is_some() {
        return Err(invalid("a comment repeats a scope tag"));
    }
    Ok(tag)
}

fn required<'a>(tags: &'a [super::Tag], name: &str) -> Result<&'a super::Tag, DomainError> {
    one(tags, name)?.ok_or_else(|| invalid("a comment is missing K or k"))
}

fn scope_author(tag: &super::Tag, declared: &str) -> Result<String, DomainError> {
    if let Some(pubkey) = tag.as_slice().get(3)
        && pubkey != declared
    {
        return Err(invalid("the scope pubkey must match the author tag"));
    }
    pubkey(declared)
}

fn parse_address(tag: &super::Tag, author: &str) -> Result<CommentScope, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid("a comment address is empty"));
    };
    let address = ReplacementAddress::from_str(value)?;
    if address.pubkey != author {
        return Err(invalid("the address pubkey must match the author tag"));
    }
    nostr_kind(&address.kind.to_string())?;
    Ok(CommentScope::Address {
        address,
        relay: hint(tag, 2, false)?,
        author: author.to_owned(),
    })
}

fn parse_event(tag: &super::Tag, author: &str) -> Result<CommentScope, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid("a comment event id is empty"));
    };
    Ok(CommentScope::Event {
        id: event_id(value)?,
        relay: hint(tag, 2, false)?,
        author: scope_author(tag, author)?,
    })
}

fn parse_external(tag: &super::Tag, kind: &str) -> Result<CommentScope, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid("an external identifier is empty"));
    };
    let derived = super::external_id::external_id_kind(value)?;
    if derived != kind {
        return Err(invalid("an external identifier does not match its kind"));
    }
    Ok(CommentScope::External {
        value: value.to_owned(),
        kind: derived,
        hint: hint(tag, 2, true)?,
    })
}

fn author_tag(tags: &[super::Tag], name: &str) -> Result<Option<String>, DomainError> {
    let Some(tag) = one(tags, name)? else {
        return Ok(None);
    };
    let Some(value) = tag.value() else {
        return Err(invalid("a comment author must be a pubkey"));
    };
    hint(tag, 2, false)?;
    Ok(Some(pubkey(value)?))
}

/// Read a kind `1111` comment.
pub fn open_comment(event: &Event) -> Result<Comment, DomainError> {
    if event.kind != COMMENT_KIND {
        return Err(invalid("a comment has kind 1111"));
    }
    comment_scopes(event)
}

/// Read NIP-22 root and parent scopes.
///
/// Kind `1244` voice replies use these tags. The kind check stays in
/// `open_comment`.
pub(crate) fn comment_scopes(event: &Event) -> Result<Comment, DomainError> {
    let tags = &event.tags;
    let root_kind = required(tags, "K")?
        .value()
        .ok_or_else(|| invalid("a comment is missing K or k"))?;
    let parent_kind = required(tags, "k")?
        .value()
        .ok_or_else(|| invalid("a comment is missing K or k"))?;
    let root_tag = ["A", "E", "I"]
        .into_iter()
        .filter_map(|name| one(tags, name).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    if root_tag.len() != 1 {
        return Err(invalid("a comment requires one root scope tag"));
    }
    let root_author = author_tag(tags, "P")?;
    let root = match root_tag[0].name() {
        Some("A") => {
            let author = root_author.ok_or_else(|| invalid("a nostr root requires a P tag"))?;
            let scope = parse_address(root_tag[0], &author)?;
            if let CommentScope::Address { address, .. } = &scope
                && address.kind.to_string() != root_kind
            {
                return Err(invalid("K must equal the root address kind"));
            }
            scope
        }
        Some("E") => {
            let author = root_author.ok_or_else(|| invalid("a nostr root requires a P tag"))?;
            nostr_kind(root_kind)?;
            parse_event(root_tag[0], &author)?
        }
        Some("I") => {
            if root_kind.parse::<u16>().is_ok() {
                return Err(invalid("an external root kind is not an event kind"));
            }
            parse_external(root_tag[0], root_kind)?
        }
        _ => return Err(invalid("a comment requires one root scope tag")),
    };

    let address = one(tags, "a")?;
    let external = one(tags, "i")?;
    let version_tags: Vec<_> = tags.iter().filter(|tag| tag.name() == Some("e")).collect();
    let parent_authors: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .map(|tag| {
            hint(tag, 2, false)?;
            let Some(value) = tag.value() else {
                return Err(invalid("a comment author must be a pubkey"));
            };
            pubkey(value)
        })
        .collect::<Result<_, _>>()?;

    let (parent, parent_version, parent_author) = if let Some(tag) = address {
        if external.is_some() || version_tags.len() != 1 {
            return Err(invalid(
                "an addressable parent requires one a tag and one e tag",
            ));
        }
        let Some(version) = version_tags[0].value() else {
            return Err(invalid("a comment event id is empty"));
        };
        let version = event_id(version)?;
        if let Some(extra) = version_tags[0].as_slice().get(3) {
            pubkey(extra)?;
        }
        hint(version_tags[0], 2, false)?;
        let Some(value) = tag.value() else {
            return Err(invalid("a comment address is empty"));
        };
        let parsed = ReplacementAddress::from_str(value)?;
        if !parent_authors.iter().any(|author| author == &parsed.pubkey) {
            return Err(invalid("a nostr parent requires a p tag"));
        }
        let scope = parse_address(tag, &parsed.pubkey)?;
        if parsed.kind.to_string() != parent_kind {
            return Err(invalid("k must equal the parent address kind"));
        }
        if let Some(extra) = version_tags[0].as_slice().get(3)
            && extra != &parsed.pubkey
        {
            return Err(invalid("the scope pubkey must match the author tag"));
        }
        let mentions = parent_authors
            .into_iter()
            .filter(|author| author != &parsed.pubkey)
            .collect();
        (scope, Some(version), mentions)
    } else if let Some(tag) = external {
        if !version_tags.is_empty() {
            return Err(invalid("an external parent has no event id"));
        }
        if parent_kind.parse::<u16>().is_ok() {
            return Err(invalid("an external parent kind is not an event kind"));
        }
        let scope = parse_external(tag, parent_kind)?;
        (scope, None, parent_authors)
    } else if version_tags.len() == 1 {
        let author = version_tags[0]
            .as_slice()
            .get(3)
            .ok_or_else(|| invalid("a parent event requires its author pubkey"))?;
        if !parent_authors.iter().any(|mention| mention == author) {
            return Err(invalid("a nostr parent requires a p tag"));
        }
        nostr_kind(parent_kind)?;
        let scope = parse_event(version_tags[0], author)?;
        let mentions = parent_authors
            .into_iter()
            .filter(|mention| mention != author)
            .collect();
        (scope, None, mentions)
    } else {
        return Err(invalid("a comment requires one parent scope tag"));
    };

    let quotes = tags
        .iter()
        .filter(|tag| tag.name() == Some("q"))
        .map(|tag| {
            let Some(value) = tag.value() else {
                return Err(invalid("a quote must be an event id or address"));
            };
            if event_id(value).is_ok() || ReplacementAddress::from_str(value).is_ok() {
                Ok(value.to_owned())
            } else {
                Err(invalid("a quote must be an event id or address"))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Comment {
        content: event.content.clone(),
        root,
        parent,
        parent_version,
        quotes,
        mentions: parent_author,
    })
}

/// A top-level comment uses the same scope for the root and the parent.
pub fn is_top_level(comment: &Comment) -> bool {
    match (&comment.root, &comment.parent) {
        (
            CommentScope::Address { address: root, .. },
            CommentScope::Address {
                address: parent, ..
            },
        ) => root == parent,
        (CommentScope::Event { id: root, .. }, CommentScope::Event { id: parent, .. }) => {
            root == parent
        }
        (
            CommentScope::External { value: root, .. },
            CommentScope::External { value: parent, .. },
        ) => root == parent,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    fn comment(tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, COMMENT_KIND, tags, content.into())
    }

    #[test]
    fn a_comment_scopes_to_the_root_including_kind_1() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/22.md"
        ))
        .unwrap();
        assert!(text.contains("kind:1111"));
        assert!(text.contains("Comments MUST point to the root scope"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "22.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "22.md")
        );

        let author = "3c9849383bdea883b0bd16fece1ed36d37e37cdde3ce43b17ea4e9192ec11289";
        let address = format!("30023:{author}:f9347ca7");
        let version = "5b4fc7fed15672fefe65d2426f67197b71ccc82aa0cc8a9e94f683eb78e07651";
        let blog = comment(
            vec![
                Tag::new(vec![
                    "A".into(),
                    address.clone(),
                    "wss://example.relay".into(),
                ]),
                Tag::new(vec!["K".into(), "30023".into()]),
                Tag::new(vec![
                    "P".into(),
                    author.into(),
                    "wss://example.relay".into(),
                ]),
                Tag::new(vec!["a".into(), address, "wss://example.relay".into()]),
                Tag::new(vec![
                    "e".into(),
                    version.into(),
                    "wss://example.relay".into(),
                ]),
                Tag::new(vec!["k".into(), "30023".into()]),
                Tag::new(vec![
                    "p".into(),
                    author.into(),
                    "wss://example.relay".into(),
                ]),
                Tag::new(vec!["p".into(), "ab".repeat(32)]),
                Tag::new(vec!["q".into(), version.into()]),
            ],
            "Great blog post!",
        );
        blog.validate_structure().unwrap();
        assert_eq!(blog.class(), EventClass::Regular);
        let opened = open_comment(&blog).unwrap();
        assert!(is_top_level(&opened));
        assert_eq!(opened.content, "Great blog post!");
        assert_eq!(opened.parent_version.as_deref(), Some(version));
        assert_eq!(opened.mentions, vec!["ab".repeat(32)]);
        assert_eq!(opened.quotes, vec![version.to_owned()]);

        let mut missing_kind = blog.clone();
        missing_kind.tags.retain(|tag| tag.name() != Some("K"));
        // The signed id no longer matches, so check the parser directly.
        missing_kind.kind = COMMENT_KIND;
        assert!(open_comment(&missing_kind).is_err());

        let kind_one = comment(
            vec![
                Tag::new(vec![
                    "E".into(),
                    version.into(),
                    "wss://example.relay".into(),
                    author.into(),
                ]),
                Tag::new(vec!["K".into(), "1".into()]),
                Tag::new(vec!["P".into(), author.into()]),
                Tag::new(vec![
                    "e".into(),
                    version.into(),
                    "wss://example.relay".into(),
                    author.into(),
                ]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["p".into(), author.into()]),
            ],
            "a kind 1 reply",
        );
        kind_one.validate_structure().unwrap();
        assert!(is_top_level(&open_comment(&kind_one).unwrap()));
        let mut missing_author = kind_one.clone();
        missing_author.tags.retain(|tag| tag.name() != Some("P"));
        assert!(open_comment(&missing_author).is_err());
        let mut reply_to_comment = kind_one.clone();
        for tag in &mut reply_to_comment.tags {
            if tag.name() == Some("k") {
                tag.0[1] = "1111".into();
            }
            if tag.name() == Some("e") {
                tag.0[1] = "ab".repeat(32);
            }
        }
        assert!(!is_top_level(&open_comment(&reply_to_comment).unwrap()));

        let web = comment(
            vec![
                Tag::new(vec!["I".into(), "https://abc.com/articles/1".into()]),
                Tag::new(vec!["K".into(), "web".into()]),
                Tag::new(vec!["i".into(), "https://abc.com/articles/1".into()]),
                Tag::new(vec!["k".into(), "web".into()]),
            ],
            "Nice article!",
        );
        let web = open_comment(&web).unwrap();
        assert!(is_top_level(&web));
        assert!(matches!(web.root, CommentScope::External { kind, .. } if kind == "web"));

        let fragment = comment(
            vec![
                Tag::new(vec!["I".into(), "https://abc.com/articles/1#part".into()]),
                Tag::new(vec!["K".into(), "web".into()]),
                Tag::new(vec!["i".into(), "https://abc.com/articles/1#part".into()]),
                Tag::new(vec!["k".into(), "web".into()]),
            ],
            "fragment",
        );
        assert!(fragment.validate_structure().is_err());

        let episode = "podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f";
        let commenter = "252f10c83610ebca1a059c0bae8255eba2f95be4d1d7bcfa89d7248a82d9f111";
        let parent_id = "80c48d992a38f9c445b943a9c9f1010b396676013443765750431a9004bdac05";
        let reply = comment(
            vec![
                Tag::new(vec![
                    "I".into(),
                    episode.into(),
                    "https://fountain.fm/episode/z1y9TMQRuqXl2awyrQxg".into(),
                ]),
                Tag::new(vec!["K".into(), "podcast:item:guid".into()]),
                Tag::new(vec![
                    "e".into(),
                    parent_id.into(),
                    "wss://example.relay".into(),
                    commenter.into(),
                ]),
                Tag::new(vec!["k".into(), "1111".into()]),
                Tag::new(vec!["p".into(), commenter.into()]),
            ],
            "I'm replying to the above comment.",
        );
        let reply = open_comment(&reply).unwrap();
        assert!(!is_top_level(&reply));
        assert!(matches!(reply.parent, CommentScope::Event { id, .. } if id == parent_id));

        let chain = comment(
            vec![
                Tag::new(vec!["I".into(), format!("bitcoin:tx:{}", "ab".repeat(32))]),
                Tag::new(vec!["K".into(), "bitcoin:tx".into()]),
                Tag::new(vec!["i".into(), format!("bitcoin:tx:{}", "ab".repeat(32))]),
                Tag::new(vec!["k".into(), "bitcoin:tx".into()]),
            ],
            "settled",
        );
        assert!(open_comment(&chain).is_ok());
        let upper = comment(
            vec![
                Tag::new(vec!["I".into(), format!("bitcoin:tx:{}", "AB".repeat(32))]),
                Tag::new(vec!["K".into(), "bitcoin:tx".into()]),
                Tag::new(vec!["i".into(), format!("bitcoin:tx:{}", "AB".repeat(32))]),
                Tag::new(vec!["k".into(), "bitcoin:tx".into()]),
            ],
            "uppercase txid",
        );
        assert!(open_comment(&upper).is_err());
    }
}
