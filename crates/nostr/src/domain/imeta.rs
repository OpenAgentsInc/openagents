//! NIP-92 `imeta` media attachment metadata.
//!
//! An `imeta` tag is variadic: each element is a space-delimited
//! `key value` pair describing a media URL in the event's content.
//! It MUST carry a `url` and at least one other field, and there
//! SHOULD be only one `imeta` per URL. Any NIP-94 field may appear.
//! NIP-92 is a draft; the `url` and second-field MUSTs hold wherever
//! the tag appears, and the tag is not added to the NIP-11 list.

use super::{DomainError, Event, Tag};

/// A parsed `imeta` tag: ordered `key value` fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imeta {
    /// The fields in tag order, key first.
    pub fields: Vec<(String, String)>,
}

impl Imeta {
    /// The first value a key carries, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// Every value a key carries, in order.
    #[must_use]
    pub fn all(&self, key: &str) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// The media URL — required by the NIP.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.get("url")
    }

    /// The MIME type, when declared.
    #[must_use]
    pub fn media_type(&self) -> Option<&str> {
        self.get("m")
    }
}

/// Parse one `imeta` tag: every element is `key value`, a `url` key is
/// present, and at least one other field exists.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for a non-`imeta` tag, an
/// element without a value, a missing `url`, or a tag holding only
/// `url`.
pub fn parse_imeta(tag: &Tag) -> Result<Imeta, DomainError> {
    if tag.name() != Some("imeta") {
        return Err(invalid("not an imeta tag"));
    }
    let mut fields = Vec::new();
    for element in tag.0.iter().skip(1) {
        let (key, value) = element
            .split_once(' ')
            .ok_or_else(|| invalid("an imeta field is a key and a value"))?;
        if key.is_empty() || value.is_empty() {
            return Err(invalid("an imeta field is a key and a value"));
        }
        fields.push((key.to_string(), value.to_string()));
    }
    let imeta = Imeta { fields };
    match imeta.url() {
        None => return Err(invalid("an imeta tag requires a url")),
        Some(url) if !is_http(url) => {
            return Err(invalid("an imeta url is an http URL"));
        }
        _ => {}
    }
    if imeta.fields.len() < 2 {
        return Err(invalid("an imeta tag requires a field beside url"));
    }
    Ok(imeta)
}

/// Parse every `imeta` tag on an event.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` on the first malformed tag.
pub fn open_imetas(event: &Event) -> Result<Vec<Imeta>, DomainError> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("imeta"))
        .map(parse_imeta)
        .collect()
}

/// Whether two `imeta` tags describe the same URL — the NIP allows
/// one tag per URL.
#[must_use]
pub fn has_duplicate_urls(imetas: &[Imeta]) -> bool {
    let mut urls = std::collections::BTreeSet::new();
    imetas
        .iter()
        .filter_map(Imeta::url)
        .any(|url| !urls.insert(url))
}

/// The `imeta` matching a URL in the event's content, if any — a
/// client may replace that URL with a rich preview.
#[must_use]
pub fn imeta_for<'a>(imetas: &'a [Imeta], event: &Event) -> Option<&'a Imeta> {
    imetas
        .iter()
        .find(|imeta| imeta.url().is_some_and(|url| event.content.contains(url)))
}

fn is_http(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.chars().any(char::is_whitespace)
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, content.to_string())
    }

    #[test]
    fn an_imeta_tag_carries_a_url_and_at_least_one_field() {
        let event = sign(
            vec![Tag::new(vec![
                "imeta".into(),
                "url https://nostr.build/i/my-image.jpg".into(),
                "m image/jpeg".into(),
                "dim 3024x4032".into(),
                "alt A scenic photo".into(),
                "fallback https://nostrcheck.me/alt1.jpg".into(),
                "fallback https://void.cat/alt2.jpg".into(),
            ])],
            "look https://nostr.build/i/my-image.jpg",
        );
        let imetas = open_imetas(&event).unwrap();
        assert_eq!(imetas.len(), 1);
        let imeta = &imetas[0];
        assert_eq!(imeta.url(), Some("https://nostr.build/i/my-image.jpg"));
        assert_eq!(imeta.media_type(), Some("image/jpeg"));
        assert_eq!(imeta.get("alt"), Some("A scenic photo"));
        assert_eq!(imeta.all("fallback").len(), 2);
        assert!(!has_duplicate_urls(&imetas));
        assert!(imeta_for(&imetas, &event).is_some());
        let elsewhere = sign(Vec::new(), "no match");
        assert!(imeta_for(&imetas, &elsewhere).is_none());
    }

    #[test]
    fn malformed_imetas_are_refused() {
        for elements in [
            vec!["imeta".to_string()],
            vec!["imeta".to_string(), "url".to_string()],
            vec!["imeta".to_string(), "m image/jpeg".to_string()],
            vec!["imeta".to_string(), "url https://x/i.jpg".to_string()],
            vec![
                "imeta".to_string(),
                "url ftp://x/i.jpg".to_string(),
                "m image/jpeg".to_string(),
            ],
            vec![
                "imeta".to_string(),
                "url https://x/i.jpg".to_string(),
                "badpair".to_string(),
            ],
        ] {
            let event = sign(vec![Tag::new(elements)], "x");
            assert!(open_imetas(&event).is_err());
        }
        let duplicates = sign(
            vec![
                Tag::new(vec![
                    "imeta".into(),
                    "url https://x/i.jpg".into(),
                    "m image/jpeg".into(),
                ]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://x/i.jpg".into(),
                    "dim 1x1".into(),
                ]),
            ],
            "x",
        );
        assert!(has_duplicate_urls(&open_imetas(&duplicates).unwrap()));
    }
}
