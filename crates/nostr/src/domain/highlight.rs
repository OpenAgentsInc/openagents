//! NIP-84 highlights.
//!
//! Kind `9802` quotes a span of text, or names non-text media with empty
//! content. The source is an `a` or `e` tag, or an `r` tag marked `source`.
//! `p` tags name authors and editors. A `comment` tag makes the event a
//! quote highlight: `p` and `r` mentions then use the `mention` marker.
//!
//! The relay stores the event and does not replace it. It does not fetch
//! the source or rewrite a URL. `clean_source_url` is the client helper
//! that drops tracker query parameters before a tag is published.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const KIND: u16 = 9_802;

/// A role on a highlight `p` tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightRole {
    Author,
    Editor,
    Mention,
}

/// One nostr event or web page the highlight points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HighlightSource {
    Address {
        address: ReplacementAddress,
        relay: Option<String>,
    },
    Event {
        id: String,
        relay: Option<String>,
    },
    Url(String),
}

/// A pubkey credited on the highlight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribution {
    pub pubkey: String,
    pub relay: Option<String>,
    pub role: Option<HighlightRole>,
}

/// A kind `9802` highlight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub content: String,
    pub sources: Vec<HighlightSource>,
    pub attributions: Vec<Attribution>,
    pub mention_pubkeys: Vec<String>,
    pub mention_urls: Vec<String>,
    pub context: Option<String>,
    /// Present when a `comment` tag makes this a quote highlight.
    pub commentary: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn role_name(value: &str) -> Option<HighlightRole> {
    match value {
        "author" => Some(HighlightRole::Author),
        "editor" => Some(HighlightRole::Editor),
        "mention" => Some(HighlightRole::Mention),
        _ => None,
    }
}

fn optional_relay(value: &str) -> Result<Option<String>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    if is_relay(value) {
        return Ok(Some(value.to_owned()));
    }
    Err(invalid("a highlight relay hint must be ws:// or wss://"))
}

fn pubkey(value: &str) -> Result<(), DomainError> {
    decode_lower_hex::<32>(value, "highlight pubkey")
        .map_err(|_| invalid("a highlight pubkey must be 32 lowercase hex bytes"))?;
    Ok(())
}

fn event_id(value: &str) -> Result<(), DomainError> {
    decode_lower_hex::<32>(value, "highlight event id")
        .map_err(|_| invalid("a highlighted event id must be 32 lowercase hex bytes"))?;
    Ok(())
}

fn is_tracker(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("utm_")
        || matches!(
            name.as_str(),
            "fbclid"
                | "gclid"
                | "dclid"
                | "msclkid"
                | "twclid"
                | "yclid"
                | "igshid"
                | "mc_eid"
                | "mc_cid"
                | "_ga"
                | "ref_src"
                | "vero_id"
                | "mkt_tok"
        )
}

/// Drop tracker query parameters from a source URL.
///
/// # Errors
///
/// Returns a sentence when `url` is not `http://` or `https://`.
pub fn clean_source_url(url: &str) -> Result<String, DomainError> {
    if !valid_http_url(url) {
        return Err(invalid("a highlight source url is http:// or https://"));
    }
    let Some((base, rest)) = url.split_once('?') else {
        return Ok(url.to_owned());
    };
    let (query, fragment) = rest
        .split_once('#')
        .map(|(query, fragment)| (query, Some(fragment)))
        .unwrap_or((rest, None));
    let kept = query
        .split('&')
        .filter(|pair| {
            let name = pair.split_once('=').map(|(name, _)| name).unwrap_or(pair);
            !pair.is_empty() && !is_tracker(name)
        })
        .collect::<Vec<_>>();
    let mut cleaned = base.to_owned();
    if !kept.is_empty() {
        cleaned.push('?');
        cleaned.push_str(&kept.join("&"));
    }
    if let Some(fragment) = fragment {
        cleaned.push('#');
        cleaned.push_str(fragment);
    }
    Ok(cleaned)
}

/// Read a kind `9802` highlight.
///
/// # Errors
///
/// Returns a sentence when a source, role, or URL marker does not match
/// the pinned text.
pub fn open_highlight(event: &Event) -> Result<Highlight, DomainError> {
    if event.kind != KIND {
        return Err(invalid("a highlight has kind 9802"));
    }
    let mut sources = Vec::new();
    let mut attributions = Vec::new();
    let mut mention_pubkeys = Vec::new();
    let mut mention_urls = Vec::new();
    for tag in &event.tags {
        match tag.name() {
            Some("a") => {
                let Some(value) = tag.value() else {
                    return Err(invalid("a highlighted address is empty"));
                };
                let address = ReplacementAddress::from_str(value)
                    .map_err(|_| invalid("a highlighted address is kind:pubkey:identifier"))?;
                let relay = match tag.as_slice().get(2) {
                    None => None,
                    Some(value) => optional_relay(value)?,
                };
                sources.push(HighlightSource::Address { address, relay });
            }
            Some("e") => {
                let Some(id) = tag.value() else {
                    return Err(invalid(
                        "a highlighted event id must be 32 lowercase hex bytes",
                    ));
                };
                event_id(id)?;
                let relay = match tag.as_slice().get(2) {
                    None => None,
                    Some(value) => optional_relay(value)?,
                };
                sources.push(HighlightSource::Event {
                    id: id.to_owned(),
                    relay,
                });
            }
            Some("r") => {
                let values = tag.as_slice();
                if values.len() != 3 || !valid_http_url(&values[1]) {
                    return Err(invalid(
                        "a highlight r tag is an http:// or https:// URL marked source or mention",
                    ));
                }
                match values[2].as_str() {
                    "source" => sources.push(HighlightSource::Url(values[1].clone())),
                    "mention" => mention_urls.push(values[1].clone()),
                    _ => {
                        return Err(invalid(
                            "a highlight r tag is an http:// or https:// URL marked source or mention",
                        ));
                    }
                }
            }
            Some("p") => {
                let values = tag.as_slice();
                let Some(key) = values.get(1) else {
                    return Err(invalid("a highlight pubkey must be 32 lowercase hex bytes"));
                };
                pubkey(key)?;
                let rest = &values[2..];
                let (relay, role) = match rest {
                    [] => (None, None),
                    [last] if role_name(last).is_some() => (None, role_name(last)),
                    [relay] => (optional_relay(relay)?, None),
                    [relay, role] => {
                        let Some(role) = role_name(role) else {
                            return Err(invalid("a highlight role is author, editor, or mention"));
                        };
                        (optional_relay(relay)?, Some(role))
                    }
                    _ => {
                        return Err(invalid(
                            "a highlight p tag is a pubkey, an optional relay, and an optional role",
                        ));
                    }
                };
                if role == Some(HighlightRole::Mention) {
                    mention_pubkeys.push(key.to_owned());
                } else {
                    attributions.push(Attribution {
                        pubkey: key.to_owned(),
                        relay,
                        role,
                    });
                }
            }
            _ => {}
        }
    }
    if sources.is_empty() {
        return Err(invalid("a highlight names an a, e, or source r tag"));
    }
    let context_tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("context"))
        .collect::<Vec<_>>();
    if context_tags.len() > 1 {
        return Err(invalid("a highlight context is one non-empty value"));
    }
    let context = match context_tags.first() {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                return Err(invalid("a highlight context is one non-empty value"));
            };
            Some(value.to_owned())
        }
    };
    let comment_tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("comment"))
        .collect::<Vec<_>>();
    if comment_tags.len() > 1 {
        return Err(invalid("a quote highlight has one comment tag"));
    }
    let commentary = match comment_tags.first() {
        None => None,
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Some(value.to_owned()),
            Some(_) => return Err(invalid("a quote highlight comment is non-empty")),
            None => Some(String::new()),
        },
    };
    Ok(Highlight {
        content: event.content.clone(),
        sources,
        attributions,
        mention_pubkeys,
        mention_urls,
        context,
        commentary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"84".repeat(32)).unwrap()
    }

    #[test]
    fn a_highlight_names_its_source_and_a_comment_quotes_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/84.md"
        ))
        .unwrap();
        assert!(text.contains("kind:9802"));
        assert!(text.contains("author"));
        assert!(text.contains("editor"));
        assert!(text.contains("mention"));
        assert!(text.contains("source"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "84.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "84.md")
        );

        let dirty = "https://example.com/essay?utm_source=newsletter&id=9&fbclid=abc#part";
        assert_eq!(
            clean_source_url(dirty).unwrap(),
            "https://example.com/essay?id=9#part"
        );
        let author = "84".repeat(32);
        let editor = "ed".repeat(32);
        let mentioned = "ae".repeat(32);
        let note = "ab".repeat(32);
        let address = format!("30023:{}:essay", signer().pubkey());
        let event = signer().sign(
            1_700_000_000,
            KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    address.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["e".into(), note.clone(), "wss://relay.example".into()]),
                Tag::new(vec![
                    "r".into(),
                    clean_source_url(dirty).unwrap(),
                    "source".into(),
                ]),
                Tag::new(vec![
                    "r".into(),
                    "https://example.com/reply".into(),
                    "mention".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    author.clone(),
                    "wss://relay.example".into(),
                    "author".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    editor.clone(),
                    "wss://relay.example".into(),
                    "editor".into(),
                ]),
                Tag::new(vec!["p".into(), mentioned.clone(), "mention".into()]),
                Tag::new(vec![
                    "context".into(),
                    "the paragraph around the span".into(),
                ]),
                Tag::new(vec!["comment".into(), "worth keeping".into()]),
            ],
            "the highlighted span".into(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Regular);
        assert!(matches!(
            compare_replacement(&event, &event),
            Err(DomainError::NotReplaceable)
        ));
        let opened = open_highlight(&event).unwrap();
        assert_eq!(opened.content, "the highlighted span");
        assert_eq!(opened.commentary.as_deref(), Some("worth keeping"));
        assert_eq!(
            opened.context.as_deref(),
            Some("the paragraph around the span")
        );
        assert!(opened.sources.iter().any(|source| matches!(
            source,
            HighlightSource::Address { address: parsed, .. } if parsed.identifier == "essay"
        )));
        assert!(opened.sources.iter().any(|source| matches!(
            source,
            HighlightSource::Event { id, .. } if id == &note
        )));
        assert!(opened.sources.iter().any(|source| matches!(
            source,
            HighlightSource::Url(url) if url == "https://example.com/essay?id=9#part"
        )));
        assert_eq!(
            opened.mention_urls,
            vec!["https://example.com/reply".to_owned()]
        );
        assert_eq!(opened.mention_pubkeys, vec![mentioned]);
        assert!(opened.attributions.iter().any(|person| {
            person.pubkey == author && person.role == Some(HighlightRole::Author)
        }));
        assert!(opened.attributions.iter().any(|person| {
            person.pubkey == editor && person.role == Some(HighlightRole::Editor)
        }));

        let media = signer().sign(
            1_700_000_100,
            KIND,
            vec![Tag::new(vec![
                "r".into(),
                "https://example.com/talk.mp4".into(),
                "source".into(),
            ])],
            String::new(),
        );
        media.validate_structure().unwrap();
        assert!(open_highlight(&media).unwrap().content.is_empty());

        let bare = signer().sign(
            1_700_000_200,
            KIND,
            vec![Tag::new(vec![
                "r".into(),
                "https://example.com/essay".into(),
            ])],
            "span".into(),
        );
        assert!(bare.validate_structure().is_err());
        let unknown_role = signer().sign(
            1_700_000_300,
            KIND,
            vec![
                Tag::new(vec!["e".into(), note]),
                Tag::new(vec![
                    "p".into(),
                    author,
                    "wss://relay.example".into(),
                    "fan".into(),
                ]),
            ],
            "span".into(),
        );
        assert!(unknown_role.validate_structure().is_err());
    }
}
