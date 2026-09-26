//! NIP-73 external content identifiers.
//!
//! An `i` tag references an external content id — a web URL, an ISBN,
//! a geohash, an ISO 3166 code, an ISAN, a DOI, a hashtag, a podcast
//! guid, or a blockchain transaction or address — and a `k` tag
//! declares its kind so clients can query every event for that kind.
//! [`external_id_kind`] classifies a value into its kind and
//! [`open_external_ids`] pairs every `i` tag with its `k`. NIP-73 is a
//! draft, so the tags are not added to the NIP-11 list.

use super::{DomainError, Event};

const GEOHASH: &str = "0123456789bcdefghjkmnpqrstuvwxyz";

/// Classify an `i` tag's value into the NIP-73 kind a matching `k`
/// tag declares.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` when the value matches none of
/// the defined identifier forms.
pub fn external_id_kind(value: &str) -> Result<String, DomainError> {
    if value.starts_with("https://") || value.starts_with("http://") {
        if !is_http(value) {
            return Err(invalid(
                "a web identifier is an http URL without a fragment",
            ));
        }
        return Ok("web".to_owned());
    }
    if let Some(id) = value.strip_prefix("isbn:") {
        return isbn(id).map(|()| "isbn".to_owned());
    }
    if let Some(id) = value.strip_prefix("geo:") {
        if id.is_empty() || !id.chars().all(|char| GEOHASH.contains(char)) {
            return Err(invalid("a geohash uses the lowercase geohash alphabet"));
        }
        return Ok("geo".to_owned());
    }
    if let Some(id) = value.strip_prefix("iso3166:") {
        // ISO 3166-1 alpha-2 (optionally alpha-3), or an ISO 3166-2
        // subdivision: the alpha-2, a hyphen, and 1-3 alphanumerics.
        let parts: Vec<&str> = id.split('-').collect();
        let country = parts[0];
        let ok = matches!(country.len(), 2 | 3)
            && country.chars().all(|char| char.is_ascii_uppercase())
            && match parts.as_slice() {
                [_] => true,
                [_, sub] => {
                    !sub.is_empty()
                        && sub.len() <= 3
                        && sub
                            .chars()
                            .all(|char| char.is_ascii_uppercase() || char.is_ascii_digit())
                }
                _ => false,
            };
        if !ok {
            return Err(invalid("an iso3166 code is uppercase letters and digits"));
        }
        return Ok("iso3166".to_owned());
    }
    if let Some(id) = value.strip_prefix("isan:") {
        if id.is_empty() || id.chars().any(char::is_whitespace) {
            return Err(invalid("an isan identifier is empty"));
        }
        return Ok("isan".to_owned());
    }
    if let Some(id) = value.strip_prefix("doi:") {
        if id.is_empty()
            || id
                .chars()
                .any(|char| char.is_ascii_uppercase() || char.is_whitespace())
        {
            return Err(invalid("a doi identifier is lowercase"));
        }
        return Ok("doi".to_owned());
    }
    if let Some(topic) = value.strip_prefix('#') {
        if topic.is_empty()
            || topic
                .chars()
                .any(|char| char.is_ascii_uppercase() || char.is_whitespace())
        {
            return Err(invalid("a hashtag is lowercase"));
        }
        return Ok("#".to_owned());
    }
    if let Some(id) = value.strip_prefix("podcast:item:guid:") {
        return guid(id).map(|()| "podcast:item:guid".to_owned());
    }
    if let Some(id) = value.strip_prefix("podcast:publisher:guid:") {
        return guid(id).map(|()| "podcast:publisher:guid".to_owned());
    }
    if let Some(id) = value.strip_prefix("podcast:guid:") {
        return guid(id).map(|()| "podcast:guid".to_owned());
    }
    blockchain(value)
}

/// An `i`/`k` pair: the external id and the kind its `k` tag declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalId {
    /// The `i` tag's value — the external identifier.
    pub value: String,
    /// The kind the value classifies into and a `k` tag declared.
    pub kind: String,
    /// The optional URL hint the `i` tag's third element carries.
    pub hint: Option<String>,
}

/// Read every `i` tag on an event, requiring each to classify into a
/// NIP-73 kind that some `k` tag on the event declares.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for an `i` value that matches
/// no defined form, an `i` whose kind no `k` declares, or a hint that
/// is not an http(s) URL.
pub fn open_external_ids(event: &Event) -> Result<Vec<ExternalId>, DomainError> {
    let declared: Vec<&str> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("k"))
        .filter_map(|tag| tag.value())
        .collect();
    let mut ids = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("i")) {
        let Some(value) = tag.value() else {
            return Err(invalid("an i tag names an external identifier"));
        };
        let kind = external_id_kind(value)?;
        if !declared.contains(&kind.as_str()) {
            return Err(invalid("an i tag needs a k tag declaring its kind"));
        }
        let hint = match tag.0.get(2) {
            None => None,
            Some(hint) if is_http(hint) => Some(hint.clone()),
            Some(_) => return Err(invalid("an i tag hint is an http URL")),
        };
        ids.push(ExternalId {
            value: value.to_string(),
            kind,
            hint,
        });
    }
    Ok(ids)
}

fn is_http(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.contains('#')
        && !value.chars().any(char::is_whitespace)
}

fn isbn(id: &str) -> Result<(), DomainError> {
    let chars: Vec<char> = id.chars().collect();
    let ok = (chars.len() == 13 && chars.iter().all(|char| char.is_ascii_digit()))
        || (chars.len() == 10
            && chars[..9].iter().all(|char| char.is_ascii_digit())
            && (chars[9].is_ascii_digit() || chars[9] == 'X'));
    if ok {
        Ok(())
    } else {
        Err(invalid(
            "an isbn identifier has 10 or 13 digits and no hyphens",
        ))
    }
}

fn guid(id: &str) -> Result<(), DomainError> {
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        Err(invalid("a podcast guid is empty"))
    } else {
        Ok(())
    }
}

fn blockchain(value: &str) -> Result<String, DomainError> {
    let parts: Vec<&str> = value.split(':').collect();
    let Some(marker_at) = parts
        .iter()
        .position(|part| *part == "tx" || *part == "address")
    else {
        return Err(invalid(
            "an external identifier is not one of the NIP-73 types",
        ));
    };
    if marker_at == 0 || marker_at > 2 || parts.len() != marker_at + 2 {
        return Err(invalid(
            "an external identifier is not one of the NIP-73 types",
        ));
    }
    let chain = parts[0];
    if chain.is_empty()
        || !chain
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit())
    {
        return Err(invalid("a blockchain name is lowercase"));
    }
    if marker_at == 2 && parts[1].is_empty() {
        return Err(invalid("a chain id is empty"));
    }
    let id = parts[marker_at + 1];
    if parts[marker_at] == "tx" {
        // The pinned Ethereum example uses its conventional 0x prefix. Keep
        // the original identifier intact while validating the payload bytes.
        let id = if chain == "ethereum" {
            id.strip_prefix("0x").unwrap_or(id)
        } else {
            id
        };
        if id.is_empty()
            || !id.len().is_multiple_of(2)
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(invalid("a transaction id is lowercase hex"));
        }
    } else if id.is_empty() || id.chars().any(char::is_whitespace) {
        return Err(invalid("a blockchain address is empty"));
    }
    Ok(format!("{chain}:{}", parts[marker_at]))
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, "referencing".to_string())
    }

    #[test]
    fn every_i_tag_classifies_and_pairs_with_its_k() {
        let event = sign(vec![
            Tag::new(vec!["i".into(), "https://example.com/post".into()]),
            Tag::new(vec!["k".into(), "web".into()]),
            Tag::new(vec!["i".into(), "isbn:9780765382030".into()]),
            Tag::new(vec!["k".into(), "isbn".into()]),
            Tag::new(vec!["i".into(), "geo:ezs42e44yx96".into()]),
            Tag::new(vec!["k".into(), "geo".into()]),
            Tag::new(vec!["i".into(), "iso3166:US-CA".into()]),
            Tag::new(vec!["k".into(), "iso3166".into()]),
            Tag::new(vec!["i".into(), "isan:0000-0000-401A-0000-7".into()]),
            Tag::new(vec!["k".into(), "isan".into()]),
            Tag::new(vec!["i".into(), "doi:10.1000/xyz123".into()]),
            Tag::new(vec!["k".into(), "doi".into()]),
            Tag::new(vec!["i".into(), "#nostr".into()]),
            Tag::new(vec!["k".into(), "#".into()]),
            Tag::new(vec![
                "i".into(),
                "podcast:item:guid:d98d189b-dc7b-45b1-8720-d4b98690f31f".into(),
                "https://fountain.fm/episode/z1y9".into(),
            ]),
            Tag::new(vec!["k".into(), "podcast:item:guid".into()]),
            Tag::new(vec!["i".into(), format!("bitcoin:tx:{}", "ab".repeat(32))]),
            Tag::new(vec!["k".into(), "bitcoin:tx".into()]),
            Tag::new(vec![
                "i".into(),
                format!("ethereum:100:tx:{}", "cd".repeat(32)),
            ]),
            Tag::new(vec!["k".into(), "ethereum:tx".into()]),
            Tag::new(vec![
                "i".into(),
                "bitcoin:address:1HQ3Go3ggs8pFnXuHVHRytPCq5fGG8Hbhx".into(),
            ]),
            Tag::new(vec!["k".into(), "bitcoin:address".into()]),
        ]);
        let ids = open_external_ids(&event).unwrap();
        assert_eq!(ids.len(), 11);
        assert_eq!(ids[0].kind, "web");
        assert_eq!(ids[8].kind, "bitcoin:tx");
        assert_eq!(ids[9].kind, "ethereum:tx");
        assert_eq!(
            external_id_kind(
                "ethereum:100:tx:0x98f7812be496f97f80e2e98d66358d1fc733cf34176a8356d171ea7fbbe97ccd"
            )
            .unwrap(),
            "ethereum:tx"
        );
        assert_eq!(ids[10].kind, "bitcoin:address");
        assert_eq!(
            ids[7].hint.as_deref(),
            Some("https://fountain.fm/episode/z1y9")
        );
        assert_eq!(
            external_id_kind("podcast:guid:c90e609a-df1e-596a-bd5e-57bcc8aad6cc").unwrap(),
            "podcast:guid"
        );
        assert_eq!(
            external_id_kind("podcast:publisher:guid:18bcbf10").unwrap(),
            "podcast:publisher:guid"
        );
    }

    #[test]
    fn malformed_identifiers_and_missing_kinds_are_refused() {
        for bad in [
            "isbn:978-0-7653-8203-0",
            "geo:EZS42",
            "iso3166:us",
            "doi:10.1000/UPPER",
            "#Upper",
            "bitcoin:tx:ABCDEF",
            "ethereum:100:tx:0x",
            "ethereum:100:tx:0xAB",
            "ethereum:100:tx:0xabc",
            "tx:abc",
            "unknown:thing",
        ] {
            assert!(external_id_kind(bad).is_err(), "{bad}");
        }
        // An i tag with no k is refused.
        let no_k = sign(vec![Tag::new(vec![
            "i".into(),
            "https://example.com".into(),
        ])]);
        assert!(open_external_ids(&no_k).is_err());
        // A k declaring the wrong kind is refused.
        let wrong_k = sign(vec![
            Tag::new(vec!["i".into(), "isbn:9780765382030".into()]),
            Tag::new(vec!["k".into(), "web".into()]),
        ]);
        assert!(open_external_ids(&wrong_k).is_err());
        // A bad hint is refused.
        let bad_hint = sign(vec![
            Tag::new(vec![
                "i".into(),
                "isbn:9780765382030".into(),
                "ftp://x".into(),
            ]),
            Tag::new(vec!["k".into(), "isbn".into()]),
        ]);
        assert!(open_external_ids(&bad_hint).is_err());
        // No i tags is fine.
        assert_eq!(open_external_ids(&sign(Vec::new())).unwrap(), vec![]);
    }
}
