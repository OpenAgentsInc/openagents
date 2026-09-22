//! NIP-23 long-form articles.
//!
//! Kind `30023` is an addressable event. One `d` tag is the article
//! identifier, so a later event from the same author replaces the older
//! one. Content is Markdown. A paragraph is one line, HTML tags are
//! refused, and `nostr:` references must be NIP-19 identifiers.
//!
//! Kind `30024` is the deprecated draft kind. It is refused. Drafts belong
//! to NIP-37. Replies to an article are kind `1111` comments.

use crate::nip19;

use super::comment::open_comment;
use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};
use std::str::FromStr;

const ARTICLE_KIND: u16 = 30_023;
const DRAFT_KIND: u16 = 30_024;

/// A kind `30023` article.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Article {
    pub identifier: String,
    pub content: String,
    pub title: Option<String>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<u64>,
    pub topics: Vec<String>,
    /// `nostr:` identifiers found in the content, without the scheme.
    pub references: Vec<String>,
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
        && !value.chars().any(char::is_whitespace)
}

fn one<'a>(tags: &'a [super::Tag], name: &str) -> Result<Option<&'a str>, DomainError> {
    let mut found = tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(invalid("an article repeats a metadata tag"));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid("an article metadata tag is empty"))
}

fn optional_text(tags: &[super::Tag], name: &str) -> Result<Option<String>, DomainError> {
    Ok(one(tags, name)?.map(str::to_owned))
}

/// Read a kind `30023` article. Kind `30024` is refused.
pub fn open_article(event: &Event) -> Result<Article, DomainError> {
    if event.kind == DRAFT_KIND {
        return Err(invalid("kind 30024 drafts moved to NIP-37"));
    }
    if event.kind != ARTICLE_KIND {
        return Err(invalid("an article has kind 30023"));
    }
    let Some(identifier) = one(&event.tags, "d")? else {
        return Err(invalid("an article requires one d tag"));
    };
    let image = optional_text(&event.tags, "image")?;
    if let Some(image) = &image
        && !is_http(image)
    {
        return Err(invalid("an article image is an http or https URL"));
    }
    let published_at = match one(&event.tags, "published_at")? {
        None => None,
        Some(value) => Some(
            value
                .parse::<u64>()
                .map_err(|_| invalid("published_at must be unix seconds"))?,
        ),
    };
    let topics = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("t"))
        .map(|tag| {
            tag.value()
                .filter(|topic| !topic.is_empty() && !topic.chars().any(char::is_whitespace))
                .map(str::to_owned)
                .ok_or_else(|| invalid("an article topic is empty"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    check_pointers(&event.tags)?;
    check_markdown(&event.content)?;
    Ok(Article {
        identifier: identifier.to_owned(),
        content: event.content.clone(),
        title: optional_text(&event.tags, "title")?,
        image,
        summary: optional_text(&event.tags, "summary")?,
        published_at,
        topics,
        references: nostr_references(&event.content)?,
    })
}

fn check_pointers(tags: &[super::Tag]) -> Result<(), DomainError> {
    for tag in tags.iter().filter(|tag| tag.name() == Some("e")) {
        let Some(id) = tag.value() else {
            return Err(invalid("an article event reference must be an event id"));
        };
        decode_lower_hex::<32>(id, "article event id")
            .map_err(|_| invalid("an article event reference must be an event id"))?;
        if let Some(relay) = tag.as_slice().get(2)
            && !relay.is_empty()
            && !is_relay(relay)
        {
            return Err(invalid("an article relay hint must be ws:// or wss://"));
        }
    }
    for tag in tags.iter().filter(|tag| tag.name() == Some("a")) {
        let Some(value) = tag.value() else {
            return Err(invalid("an article address reference is empty"));
        };
        ReplacementAddress::from_str(value)?;
        if let Some(relay) = tag.as_slice().get(2)
            && !relay.is_empty()
            && !is_relay(relay)
        {
            return Err(invalid("an article relay hint must be ws:// or wss://"));
        }
    }
    Ok(())
}

fn check_markdown(content: &str) -> Result<(), DomainError> {
    let mut in_fence = false;
    let mut previous_prose = false;
    for line in content.split('\n') {
        let line = line.trim_end_matches('\r').trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            previous_prose = false;
            continue;
        }
        if in_fence || line.is_empty() || structural(line) {
            previous_prose = false;
            continue;
        }
        if previous_prose {
            return Err(invalid("a paragraph must not contain a hard line break"));
        }
        if contains_html(line) {
            return Err(invalid("an article must not contain HTML"));
        }
        previous_prose = true;
    }
    if in_fence {
        return Err(invalid("an article code fence is not closed"));
    }
    Ok(())
}

fn structural(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    matches!(bytes[0], b'#' | b'>' | b'|')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line.starts_with("---")
        || line
            .split_once(". ")
            .is_some_and(|(index, _)| index.chars().all(|char| char.is_ascii_digit()))
}

fn contains_html(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'<' {
            continue;
        }
        let rest = &line[index + 1..];
        if rest.starts_with("https://") || rest.starts_with("http://") {
            continue;
        }
        let name = rest.strip_prefix('/').unwrap_or(rest);
        if name.starts_with("!--") || name.starts_with(|char: char| char.is_ascii_alphabetic()) {
            return true;
        }
    }
    false
}

fn nostr_references(content: &str) -> Result<Vec<String>, DomainError> {
    let mut references = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("nostr:") {
        rest = &rest[start + "nostr:".len()..];
        let end = rest
            .find(|char: char| !char.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        let identifier = &rest[..end];
        if identifier.is_empty() || identifier.starts_with("nsec") {
            return Err(invalid("an article reference must be a nostr: identifier"));
        }
        nip19::decode(identifier)
            .map_err(|_| invalid("an article reference must be a nostr: identifier"))?;
        references.push(identifier.to_owned());
        rest = &rest[end..];
    }
    Ok(references)
}

/// Whether `event` is a kind `1111` comment on a kind `30023` article.
pub fn is_article_reply(event: &Event) -> bool {
    event.kind == 1_111
        && open_comment(event).is_ok()
        && event
            .tag_values("K")
            .chain(event.tag_values("k"))
            .any(|kind| kind == "30023")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};
    use crate::nip19::encode_npub;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    #[test]
    fn an_article_replaces_on_its_identifier_and_keeps_markdown_paragraphs() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/23.md"
        ))
        .unwrap();
        assert!(text.contains("30023"));
        assert!(text.contains("kind:30024"));
        assert!(text.contains("kind 1111"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "23.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "23.md")
        );

        let npub = encode_npub(&[0x11; 32]);
        let author = "a695f6b60119d9521934a691347d9f78e8770b56da16bb255ee286ddf9fda919";
        let content = format!(
            "Lorem ipsum nostr:{npub} dolor sit amet.\n\nRead more at <https://example.com/article>."
        );
        let article = signer().sign(
            1_675_642_635,
            ARTICLE_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["title".into(), "Lorem Ipsum".into()]),
                Tag::new(vec!["published_at".into(), "1296962229".into()]),
                Tag::new(vec!["summary".into(), "A placeholder.".into()]),
                Tag::new(vec!["image".into(), "https://example.com/cover.png".into()]),
                Tag::new(vec!["t".into(), "placeholder".into()]),
                Tag::new(vec![
                    "e".into(),
                    "b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87".into(),
                    "wss://relay.example.com".into(),
                ]),
                Tag::new(vec![
                    "a".into(),
                    format!("30023:{author}:ipsum"),
                    "wss://relay.nostr.org".into(),
                ]),
            ],
            content.clone(),
        );
        article.validate_structure().unwrap();
        assert_eq!(article.class(), EventClass::Addressable);
        let opened = open_article(&article).unwrap();
        assert_eq!(opened.identifier, "lorem-ipsum");
        assert_eq!(opened.title.as_deref(), Some("Lorem Ipsum"));
        assert_eq!(opened.published_at, Some(1_296_962_229));
        assert_eq!(opened.topics, vec!["placeholder".to_owned()]);
        assert_eq!(opened.references, vec![npub]);

        let newer = signer().sign(
            1_675_642_700,
            ARTICLE_KIND,
            vec![Tag::new(vec!["d".into(), "lorem-ipsum".into()])],
            "A revised paragraph.\n\nStill one line each.".into(),
        );
        assert_eq!(
            compare_replacement(&article, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let wrapped = signer().sign(
            1_675_642_635,
            ARTICLE_KIND,
            vec![Tag::new(vec!["d".into(), "wrapped".into()])],
            "This line is wrapped at a column boundary so the next\nline continues the same paragraph.".into(),
        );
        assert!(wrapped.validate_structure().is_err());

        let html = signer().sign(
            1_675_642_635,
            ARTICLE_KIND,
            vec![Tag::new(vec!["d".into(), "html".into()])],
            "Hello <b>bold</b> text.".into(),
        );
        assert!(html.validate_structure().is_err());

        let draft = signer().sign(
            1_675_642_635,
            DRAFT_KIND,
            vec![Tag::new(vec!["d".into(), "draft".into()])],
            "A deprecated draft.".into(),
        );
        assert!(draft.validate_structure().is_err());

        let missing_d = signer().sign(
            1_675_642_635,
            ARTICLE_KIND,
            Vec::new(),
            "No identifier.".into(),
        );
        assert!(missing_d.validate_structure().is_err());

        let commenter = RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
        let reply = commenter.sign(
            1_675_642_800,
            1_111,
            vec![
                Tag::new(vec![
                    "A".into(),
                    format!("30023:{}:lorem-ipsum", signer().pubkey()),
                    "wss://relay.example.com".into(),
                ]),
                Tag::new(vec!["K".into(), "30023".into()]),
                Tag::new(vec!["P".into(), signer().pubkey().into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("30023:{}:lorem-ipsum", signer().pubkey()),
                    "wss://relay.example.com".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    article.id.clone(),
                    "wss://relay.example.com".into(),
                ]),
                Tag::new(vec!["k".into(), "30023".into()]),
                Tag::new(vec!["p".into(), signer().pubkey().into()]),
            ],
            "A comment, not a kind 1 reply.".into(),
        );
        assert!(is_article_reply(&reply));
        let note = signer().sign(1_675_642_800, 1, Vec::new(), "not a reply".into());
        assert!(!is_article_reply(&note));
    }
}
