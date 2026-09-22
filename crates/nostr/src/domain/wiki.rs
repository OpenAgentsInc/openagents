//! NIP-54 wiki articles.
//!
//! Kind `30818` is an addressable article. Its `d` tag is a normalized
//! subject name: letters are lowercase, whitespace is `-`, and
//! punctuation is removed. Numbers and non-ASCII letters stay. The
//! content is Djot. A reference link with no definition becomes a
//! wikilink to that normalized name. A defined reference stays a URI.
//!
//! Kind `818` asks the destination author to merge one article version
//! into another. Kind `30819` redirects one normalized name to an
//! article. A newer article or redirect with the same `d` tag replaces
//! the older one. A merge request does not replace.
//!
//! Djot is not rendered. The merge source event is not fetched, so its
//! kind is not checked. Reactions, relay lists, and contact lists are
//! not ranked. NIP-54 is a draft, so these kinds stay off the NIP-11
//! list.

use std::collections::BTreeMap;
use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const ARTICLE_KIND: u16 = 30_818;
const MERGE_KIND: u16 = 818;
const REDIRECT_KIND: u16 = 30_819;

/// Where a wiki link points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WikiTarget {
    /// A normalized article name.
    Article { identifier: String },
    /// A `nostr:` URI or another explicit target.
    Uri { uri: String },
}

/// One link extracted from article content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLink {
    pub display: String,
    pub target: WikiTarget,
}

/// The article and version a fork or defer names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiVersion {
    pub address: ReplacementAddress,
    pub event_id: String,
    pub relay: Option<String>,
}

/// A kind `30818` article.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiArticle {
    pub identifier: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub fork: Option<WikiVersion>,
    pub defer: Option<WikiVersion>,
    pub links: Vec<WikiLink>,
    pub content: String,
}

/// A kind `818` merge request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiMerge {
    pub target: ReplacementAddress,
    pub target_relay: Option<String>,
    pub base: Option<String>,
    pub source: String,
    pub source_relay: Option<String>,
    pub destination: String,
    pub content: String,
}

/// A kind `30819` redirect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiRedirect {
    pub identifier: String,
    pub target: ReplacementAddress,
    pub relay: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

/// Normalize a subject into a wiki `d` tag.
pub fn normalize_wiki_identifier(value: &str) -> String {
    let mut raw = String::new();
    for ch in value.chars() {
        if ch.is_whitespace() || ch == '-' {
            raw.push('-');
        } else if ch.is_alphabetic() {
            for lower in ch.to_lowercase() {
                if lower.is_alphabetic() || lower.is_numeric() {
                    raw.push(lower);
                } else if lower.is_whitespace() {
                    raw.push('-');
                }
            }
        } else if ch.is_numeric() {
            raw.push(ch);
        }
    }
    let mut collapsed = String::new();
    let mut hyphen = false;
    for ch in raw.chars() {
        if ch == '-' {
            if !hyphen && !collapsed.is_empty() {
                collapsed.push('-');
                hyphen = true;
            }
        } else {
            collapsed.push(ch);
            hyphen = false;
        }
    }
    if collapsed.ends_with('-') {
        collapsed.pop();
    }
    collapsed
}

fn normalized_identifier(value: &str) -> Result<String, DomainError> {
    let normalized = normalize_wiki_identifier(value);
    if normalized.is_empty() || normalized != value {
        return Err(invalid("a wiki identifier is already normalized"));
    }
    Ok(normalized)
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<Option<&'a str>, DomainError> {
    let mut found = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(invalid(reason));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid(reason))
}

fn text(value: &str, reason: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.len() > 4_096 || value.chars().any(char::is_control) {
        return Err(invalid(reason));
    }
    Ok(value.to_owned())
}

fn relay(value: &str) -> Result<Option<String>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    if (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
    {
        Ok(Some(value.to_owned()))
    } else {
        Err(invalid("a wiki relay is ws:// or wss://"))
    }
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "wiki event id")
        .map_err(|_| invalid("a wiki event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn article_address(value: &str) -> Result<ReplacementAddress, DomainError> {
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a wiki address names an article"))?;
    if address.kind != ARTICLE_KIND {
        return Err(invalid("a wiki address names an article"));
    }
    normalized_identifier(&address.identifier)?;
    Ok(address)
}

fn reference_definition(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let rest = line.strip_prefix('[')?;
    let end = rest.find("]:")?;
    let label = &rest[..end];
    if label.is_empty() {
        return None;
    }
    let target = rest[end + 2..].trim();
    if target.is_empty() || target.chars().any(char::is_whitespace) {
        return None;
    }
    Some((label.to_owned(), target.to_owned()))
}

fn take_bracket(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start) != Some(&'[') {
        return None;
    }
    let end = chars[start + 1..].iter().position(|ch| *ch == ']')?;
    let text: String = chars[start + 1..start + 1 + end].iter().collect();
    Some((text, start + 1 + end + 1))
}

fn wiki_links(content: &str) -> Result<Vec<WikiLink>, DomainError> {
    let mut definitions = BTreeMap::new();
    for line in content.lines() {
        if let Some((label, target)) = reference_definition(line)
            && definitions.insert(label, target).is_some()
        {
            return Err(invalid("a wiki reference is defined once"));
        }
    }
    let mut links = Vec::new();
    for line in content.lines() {
        if reference_definition(line).is_some() {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            if chars[index] != '[' {
                index += 1;
                continue;
            }
            let Some((display, after_display)) = take_bracket(&chars, index) else {
                break;
            };
            if after_display >= chars.len() {
                break;
            }
            if chars[after_display] == '(' {
                let Some(close) = chars[after_display + 1..].iter().position(|ch| *ch == ')')
                else {
                    return Err(invalid("a wiki link has a target"));
                };
                let target: String = chars[after_display + 1..after_display + 1 + close]
                    .iter()
                    .collect();
                if display.is_empty()
                    || target.is_empty()
                    || target.chars().any(char::is_whitespace)
                {
                    return Err(invalid("a wiki link has text and a target"));
                }
                links.push(WikiLink {
                    display,
                    target: WikiTarget::Uri { uri: target },
                });
                index = after_display + 1 + close + 1;
            } else if chars[after_display] == '[' {
                let Some((label, after_label)) = take_bracket(&chars, after_display) else {
                    return Err(invalid("a wiki link has a target"));
                };
                if display.is_empty() {
                    return Err(invalid("a wiki link has text"));
                }
                let key = if label.is_empty() {
                    display.clone()
                } else {
                    label
                };
                if let Some(uri) = definitions.get(&key) {
                    links.push(WikiLink {
                        display,
                        target: WikiTarget::Uri { uri: uri.clone() },
                    });
                } else {
                    let identifier = normalize_wiki_identifier(&key);
                    if identifier.is_empty() {
                        return Err(invalid("a wikilink has a name"));
                    }
                    links.push(WikiLink {
                        display,
                        target: WikiTarget::Article { identifier },
                    });
                }
                index = after_label;
            } else {
                index += 1;
            }
        }
    }
    Ok(links)
}

fn marked_version(event: &Event, marker: &str) -> Result<Option<WikiVersion>, DomainError> {
    let mut address = None;
    let mut address_relay = None;
    let mut id = None;
    let mut id_relay = None;
    for tag in &event.tags {
        let parts = tag.as_slice();
        let name = tag.name();
        if !matches!(name, Some("a" | "e")) {
            continue;
        }
        let found = parts.get(3).map(String::as_str);
        if found != Some(marker) {
            continue;
        }
        if parts.len() != 4 {
            return Err(invalid("a wiki reference is an address or an event id"));
        }
        let hint = relay(&parts[2])?;
        match name {
            Some("a") => {
                if address.is_some() {
                    return Err(invalid("a wiki reference names one article"));
                }
                address = Some(article_address(&parts[1])?);
                address_relay = hint;
            }
            Some("e") => {
                if id.is_some() {
                    return Err(invalid("a wiki reference names one version"));
                }
                id = Some(event_id(&parts[1])?);
                id_relay = hint;
            }
            _ => {}
        }
    }
    match (address, id) {
        (None, None) => Ok(None),
        (Some(address), Some(event_id)) => Ok(Some(WikiVersion {
            address,
            event_id,
            relay: address_relay.or(id_relay),
        })),
        _ => Err(invalid(
            "a wiki reference names the article and the version",
        )),
    }
}

fn refuse_unmarked(event: &Event) -> Result<(), DomainError> {
    for tag in event
        .tags
        .iter()
        .filter(|tag| matches!(tag.name(), Some("a" | "e")))
    {
        match tag.as_slice().get(3).map(String::as_str) {
            Some("fork" | "defer") => {}
            _ => return Err(invalid("a wiki reference is fork or defer")),
        }
    }
    Ok(())
}

/// Read a kind `30818` article.
pub fn open_wiki_article(event: &Event) -> Result<WikiArticle, DomainError> {
    if event.kind != ARTICLE_KIND {
        return Err(invalid("a wiki article has kind 30818"));
    }
    if event.content.is_empty() {
        return Err(invalid("a wiki article has text"));
    }
    refuse_unmarked(event)?;
    Ok(WikiArticle {
        identifier: normalized_identifier(
            one(event, "d", "a wiki article has one identifier")?
                .ok_or_else(|| invalid("a wiki article has one identifier"))?,
        )?,
        title: one(event, "title", "a wiki title is text")?
            .map(|value| text(value, "a wiki title is text"))
            .transpose()?,
        summary: one(event, "summary", "a wiki summary is text")?
            .map(|value| text(value, "a wiki summary is text"))
            .transpose()?,
        fork: marked_version(event, "fork")?,
        defer: marked_version(event, "defer")?,
        links: wiki_links(&event.content)?,
        content: event.content.clone(),
    })
}

/// Read a kind `818` merge request.
pub fn open_wiki_merge(event: &Event) -> Result<WikiMerge, DomainError> {
    if event.kind != MERGE_KIND {
        return Err(invalid("a wiki merge has kind 818"));
    }
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect();
    if tags.len() != 1 || tags[0].as_slice().len() < 2 || tags[0].as_slice().len() > 3 {
        return Err(invalid("a wiki merge names one article"));
    }
    let target = article_address(&tags[0].as_slice()[1])?;
    let target_relay = match tags[0].as_slice().get(2) {
        None => None,
        Some(value) => relay(value)?,
    };
    let mut base = None;
    let mut source = None;
    let mut source_relay = None;
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("e")) {
        let parts = tag.as_slice();
        match parts.get(3).map(String::as_str) {
            Some("source") if parts.len() == 4 => {
                if source.is_some() {
                    return Err(invalid("a wiki merge names one source"));
                }
                source = Some(event_id(&parts[1])?);
                source_relay = relay(&parts[2])?;
            }
            None if parts.len() == 2 || parts.len() == 3 => {
                if base.is_some() {
                    return Err(invalid("a wiki merge names one base version"));
                }
                base = Some(event_id(&parts[1])?);
                if let Some(value) = parts.get(2) {
                    relay(value)?;
                }
            }
            _ => return Err(invalid("a wiki merge source is marked source")),
        }
    }
    let Some(source) = source else {
        return Err(invalid("a wiki merge names one source"));
    };
    let destinations: Vec<_> = event.tag_values("p").collect();
    if destinations.len() != 1 || destinations[0] != target.pubkey {
        return Err(invalid("a wiki merge names the destination author"));
    }
    Ok(WikiMerge {
        target,
        target_relay,
        base,
        source,
        source_relay,
        destination: destinations[0].to_owned(),
        content: event.content.clone(),
    })
}

/// Read a kind `30819` redirect.
pub fn open_wiki_redirect(event: &Event) -> Result<WikiRedirect, DomainError> {
    if event.kind != REDIRECT_KIND {
        return Err(invalid("a wiki redirect has kind 30819"));
    }
    let identifier = normalized_identifier(
        one(event, "d", "a wiki redirect has one identifier")?
            .ok_or_else(|| invalid("a wiki redirect has one identifier"))?,
    )?;
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect();
    if tags.len() != 1 || tags[0].as_slice().len() < 2 || tags[0].as_slice().len() > 3 {
        return Err(invalid("a wiki redirect names one article"));
    }
    let target = article_address(&tags[0].as_slice()[1])?;
    if target.pubkey == event.pubkey && target.identifier == identifier {
        return Err(invalid("a wiki redirect names another article"));
    }
    let relay = match tags[0].as_slice().get(2) {
        None => None,
        Some(value) => relay(value)?,
    };
    Ok(WikiRedirect {
        identifier,
        target,
        relay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_wiki_article_normalizes_its_name_and_a_merge_names_the_fork() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/54.md"
        ))
        .unwrap();
        assert!(text.contains("30818"));
        assert!(text.contains("30819"));
        assert!(text.contains("kind:818"));
        assert!(text.contains("wiki-article"));
        assert!(text.contains("whats-up"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "54.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "54.md")
        );

        assert_eq!(normalize_wiki_identifier("Wiki Article"), "wiki-article");
        assert_eq!(normalize_wiki_identifier("What's Up?"), "whats-up");
        assert_eq!(normalize_wiki_identifier("  Hello  World  "), "hello-world");
        assert_eq!(normalize_wiki_identifier("Article 1"), "article-1");
        assert_eq!(
            normalize_wiki_identifier("ウィキペディア"),
            "ウィキペディア"
        );
        assert_eq!(normalize_wiki_identifier("Ñoño"), "ñoño");
        assert_eq!(normalize_wiki_identifier("Москва"), "москва");
        assert_eq!(
            normalize_wiki_identifier("日本語 Article"),
            "日本語-article"
        );

        let author = signer("54");
        let content = "\
Bitcoin is a [cryptocurrency][] invented by [Satoshi Nakamoto][].

See also: [proof of work][] and [lightning network][Lightning Network].

[Satoshi Nakamoto]: nostr:npub1satoshi
";
        let article = author.sign(
            1_700_000_000,
            ARTICLE_KIND,
            vec![
                Tag::new(vec!["d".into(), "wiki".into()]),
                Tag::new(vec!["title".into(), "Wiki".into()]),
            ],
            content.into(),
        );
        article.validate_structure().unwrap();
        assert_eq!(article.class(), EventClass::Addressable);
        let opened = open_wiki_article(&article).unwrap();
        assert_eq!(opened.identifier, "wiki");
        assert_eq!(opened.title.as_deref(), Some("Wiki"));
        assert_eq!(
            opened.links,
            vec![
                WikiLink {
                    display: "cryptocurrency".into(),
                    target: WikiTarget::Article {
                        identifier: "cryptocurrency".into(),
                    },
                },
                WikiLink {
                    display: "Satoshi Nakamoto".into(),
                    target: WikiTarget::Uri {
                        uri: "nostr:npub1satoshi".into(),
                    },
                },
                WikiLink {
                    display: "proof of work".into(),
                    target: WikiTarget::Article {
                        identifier: "proof-of-work".into(),
                    },
                },
                WikiLink {
                    display: "lightning network".into(),
                    target: WikiTarget::Article {
                        identifier: "lightning-network".into(),
                    },
                },
            ]
        );

        let revised = author.sign(
            1_700_000_100,
            ARTICLE_KIND,
            vec![
                Tag::new(vec!["d".into(), "wiki".into()]),
                Tag::new(vec!["summary".into(), "A short description".into()]),
            ],
            "A wiki is a hypertext publication.".into(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&article, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let editor = signer("55");
        let forked = editor.sign(
            1_700_000_200,
            ARTICLE_KIND,
            vec![
                Tag::new(vec!["d".into(), "wiki".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("30818:{}:wiki", author.pubkey()),
                    "wss://relay.example".into(),
                    "fork".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    article.id.clone(),
                    "wss://relay.example".into(),
                    "fork".into(),
                ]),
            ],
            "I added information about the block size limit.".into(),
        );
        forked.validate_structure().unwrap();
        let fork = open_wiki_article(&forked).unwrap().fork.unwrap();
        assert_eq!(fork.address.pubkey, author.pubkey());
        assert_eq!(fork.address.identifier, "wiki");
        assert_eq!(fork.event_id, article.id);

        let merge = editor.sign(
            1_700_000_300,
            MERGE_KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    format!("30818:{}:wiki", author.pubkey()),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    article.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec![
                    "e".into(),
                    forked.id.clone(),
                    "wss://relay.example".into(),
                    "source".into(),
                ]),
            ],
            "I added information about the block size limit".into(),
        );
        merge.validate_structure().unwrap();
        assert_eq!(merge.class(), EventClass::Regular);
        let request = open_wiki_merge(&merge).unwrap();
        assert_eq!(request.destination, author.pubkey());
        assert_eq!(request.source, forked.id);
        assert_eq!(request.base.as_deref(), Some(article.id.as_str()));
        assert!(matches!(
            compare_replacement(&merge, &merge),
            Err(DomainError::NotReplaceable)
        ));

        let redirect = author.sign(
            1_700_000_400,
            REDIRECT_KIND,
            vec![
                Tag::new(vec!["d".into(), "btc".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("30818:{}:wiki", author.pubkey()),
                    "wss://relay.example".into(),
                ]),
            ],
            String::new(),
        );
        redirect.validate_structure().unwrap();
        assert_eq!(redirect.class(), EventClass::Addressable);
        let opened_redirect = open_wiki_redirect(&redirect).unwrap();
        assert_eq!(opened_redirect.identifier, "btc");
        assert_eq!(opened_redirect.target.identifier, "wiki");
        let moved = author.sign(
            1_700_000_500,
            REDIRECT_KIND,
            vec![
                Tag::new(vec!["d".into(), "btc".into()]),
                Tag::new(vec!["a".into(), format!("30818:{}:wiki", editor.pubkey())]),
            ],
            String::new(),
        );
        moved.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&redirect, &moved),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let deferred = editor.sign(
            1_700_000_600,
            ARTICLE_KIND,
            vec![
                Tag::new(vec!["d".into(), "old-notes".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("30818:{}:wiki", author.pubkey()),
                    String::new(),
                    "defer".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    article.id.clone(),
                    String::new(),
                    "defer".into(),
                ]),
            ],
            "The original article is better.".into(),
        );
        deferred.validate_structure().unwrap();
        let better = open_wiki_article(&deferred).unwrap().defer.unwrap();
        assert_eq!(better.address.identifier, "wiki");
        assert_eq!(better.event_id, article.id);

        let rough = author.sign(
            1_700_000_700,
            ARTICLE_KIND,
            vec![Tag::new(vec!["d".into(), "Wiki Article".into()])],
            "Not normalized.".into(),
        );
        assert!(rough.validate_structure().is_err());
        let unmarked = editor.sign(
            1_700_000_800,
            MERGE_KIND,
            vec![
                Tag::new(vec!["a".into(), format!("30818:{}:wiki", author.pubkey())]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), forked.id.clone()]),
            ],
            String::new(),
        );
        assert!(unmarked.validate_structure().is_err());
    }
}
