//! NIP-89 application handlers.
//!
//! Kind `31989` recommends handlers for one event kind. Kind `31990`
//! describes one application: the kinds it supports, optional profile
//! metadata, optional site-manifest pointers, and platform URLs. A URL
//! contains the literal `bech32`, which the client replaces with a NIP-19
//! token. A `client` tag on another event names the handler that published
//! it. Omitting that tag is the opt-out.
//!
//! The relay stores both kinds on their `d` tag and does not fetch the
//! application or its kind `0` profile. This crate encodes `npub` and
//! `nsec` only, so the caller supplies the NIP-19 token.

use std::str::FromStr;

use super::{DomainError, Event, ReplacementAddress};

const RECOMMENDATION_KIND: u16 = 31_989;
const HANDLER_KIND: u16 = 31_990;

/// One recommended kind `31990` handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecommendedHandler {
    pub handler: ReplacementAddress,
    pub relay: Option<String>,
    pub platform: Option<String>,
}

/// A kind `31989` recommendation for one event kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandlerRecommendation {
    pub kind: u16,
    pub handlers: Vec<RecommendedHandler>,
}

/// A `latest` or `next` site-manifest pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestRef {
    pub address: ReplacementAddress,
    pub relay: Option<String>,
}

/// One platform URL. `entity` is empty when the URL is a generic handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandlerLink {
    pub platform: String,
    pub url: String,
    pub entity: Option<String>,
}

/// A kind `31990` application handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHandler {
    pub identifier: String,
    pub kinds: Vec<u16>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    pub latest: Option<ManifestRef>,
    pub next: Option<ManifestRef>,
    pub links: Vec<HandlerLink>,
}

/// A `client` tag naming the handler that published an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientAttribution {
    pub name: String,
    pub handler: ReplacementAddress,
    pub relay: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn is_platform(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && (2..=16).contains(&value.len())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn optional_relay(value: &str) -> Result<Option<String>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    if is_relay(value) {
        return Ok(Some(value.to_owned()));
    }
    Err(invalid("a handler relay hint must be ws:// or wss://"))
}

fn kind_token(value: &str) -> Result<u16, DomainError> {
    let kind = value
        .parse::<u16>()
        .map_err(|_| invalid("a handler kind is an event kind"))?;
    if kind.to_string() != value {
        return Err(invalid("a handler kind is an event kind"));
    }
    Ok(kind)
}

fn check_identifier(value: &str) -> Result<(), DomainError> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_whitespace) {
        return Err(invalid(
            "a handler identifier contains 1 to 256 characters and no whitespace",
        ));
    }
    Ok(())
}

fn one_tag<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid(reason));
    }
    tags[0]
        .value()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(reason))
}

fn handler_address(value: &str) -> Result<ReplacementAddress, DomainError> {
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a handler address is 31990:pubkey:identifier"))?;
    if address.kind != HANDLER_KIND || address.identifier.is_empty() {
        return Err(invalid("a handler address is 31990:pubkey:identifier"));
    }
    check_identifier(&address.identifier)?;
    Ok(address)
}

fn manifest_address(value: &str) -> Result<ReplacementAddress, DomainError> {
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a site manifest address is kind:pubkey:identifier"))?;
    if address.identifier.is_empty() {
        return Err(invalid("a site manifest address is kind:pubkey:identifier"));
    }
    Ok(address)
}

/// Read a kind `31989` recommendation.
///
/// # Errors
///
/// Returns a sentence when the kind, the `d` tag, or an `a` tag is refused.
pub fn open_recommendation(event: &Event) -> Result<HandlerRecommendation, DomainError> {
    if event.kind != RECOMMENDATION_KIND {
        return Err(invalid("a handler recommendation has kind 31989"));
    }
    let kind = kind_token(one_tag(
        event,
        "d",
        "a handler recommendation has one kind in its d tag",
    )?)?;
    let mut handlers = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 4 {
            return Err(invalid(
                "a recommendation names a 31990 address, an optional relay, and an optional platform",
            ));
        }
        let handler = handler_address(&values[1])?;
        let (relay, platform) = match values.len() {
            2 => (None, None),
            3 if is_relay(&values[2]) => (Some(values[2].clone()), None),
            3 if is_platform(&values[2]) => (None, Some(values[2].clone())),
            3 => {
                return Err(invalid(
                    "a recommendation names a 31990 address, an optional relay, and an optional platform",
                ));
            }
            _ => {
                if !is_platform(&values[3]) {
                    return Err(invalid(
                        "a recommendation platform is a short lowercase name",
                    ));
                }
                (optional_relay(&values[2])?, Some(values[3].clone()))
            }
        };
        handlers.push(RecommendedHandler {
            handler,
            relay,
            platform,
        });
    }
    if handlers.is_empty() {
        return Err(invalid(
            "a handler recommendation names at least one application",
        ));
    }
    Ok(HandlerRecommendation { kind, handlers })
}

fn manifest_tag(event: &Event, name: &str) -> Result<Option<ManifestRef>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid("a handler has one latest tag and one next tag"));
    }
    let Some(tag) = tags.first() else {
        return Ok(None);
    };
    let values = tag.as_slice();
    if values.len() < 2 || values.len() > 3 {
        return Err(invalid("a site manifest address is kind:pubkey:identifier"));
    }
    let address = manifest_address(&values[1])?;
    let relay = match values.get(2) {
        None => None,
        Some(value) => optional_relay(value)?,
    };
    Ok(Some(ManifestRef { address, relay }))
}

/// Read a kind `31990` handler.
///
/// # Errors
///
/// Returns a sentence when the identifier, a kind, or a platform URL is refused.
pub fn open_handler(event: &Event) -> Result<AppHandler, DomainError> {
    if event.kind != HANDLER_KIND {
        return Err(invalid("an application handler has kind 31990"));
    }
    let identifier = one_tag(event, "d", "an application handler has one d tag")?.to_owned();
    check_identifier(&identifier)?;
    let mut kinds = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("k")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a handler kind is an event kind"));
        };
        kinds.push(kind_token(value)?);
    }
    if kinds.is_empty() {
        return Err(invalid("an application handler names at least one kind"));
    }
    let metadata = if event.content.is_empty() {
        None
    } else {
        let value: serde_json::Value = serde_json::from_str(&event.content)
            .map_err(|_| invalid("handler metadata is a JSON object"))?;
        let Some(object) = value.as_object() else {
            return Err(invalid("handler metadata is a JSON object"));
        };
        Some(object.clone())
    };
    let mut links = Vec::new();
    for tag in &event.tags {
        let Some(name) = tag.name() else {
            continue;
        };
        if matches!(name, "d" | "k" | "latest" | "next" | "client") || !is_platform(name) {
            continue;
        }
        let values = tag.as_slice();
        if values.len() < 2
            || values.len() > 3
            || !values[1].contains("bech32")
            || values[1].chars().any(char::is_whitespace)
            || values[1].len() > 2_048
        {
            return Err(invalid(
                "a platform URL contains the bech32 placeholder and an optional NIP-19 type",
            ));
        }
        let entity = match values.get(2) {
            None => None,
            Some(value) if is_platform(value) => Some(value.clone()),
            Some(_) => {
                return Err(invalid(
                    "a platform URL contains the bech32 placeholder and an optional NIP-19 type",
                ));
            }
        };
        links.push(HandlerLink {
            platform: name.to_owned(),
            url: values[1].clone(),
            entity,
        });
    }
    Ok(AppHandler {
        identifier,
        kinds,
        metadata,
        latest: manifest_tag(event, "latest")?,
        next: manifest_tag(event, "next")?,
        links,
    })
}

/// The platform link for `entity`, or the generic link on that platform.
pub fn link_for<'a>(
    handler: &'a AppHandler,
    platform: &str,
    entity: Option<&str>,
) -> Option<&'a HandlerLink> {
    let mut links = handler
        .links
        .iter()
        .filter(|link| link.platform == platform);
    if let Some(entity) = entity
        && let Some(link) = links
            .clone()
            .find(|link| link.entity.as_deref() == Some(entity))
    {
        return Some(link);
    }
    links.find(|link| link.entity.is_none())
}

/// Replace every `bech32` placeholder with a NIP-19 token.
///
/// # Errors
///
/// Returns a sentence when the template has no placeholder or the token
/// does not decode.
pub fn handler_url(template: &str, nip19_entity: &str) -> Result<String, DomainError> {
    crate::nip19::decode(nip19_entity)
        .map_err(|_| invalid("a handler entity is a NIP-19 token"))?;
    if !template.contains("bech32") {
        return Err(invalid("a handler URL contains the bech32 placeholder"));
    }
    Ok(template.replace("bech32", nip19_entity))
}

/// Read one `client` tag. `Ok(None)` is the opt-out.
///
/// # Errors
///
/// Returns a sentence when a `client` tag is present and malformed.
pub fn open_client_tag(event: &Event) -> Result<Option<ClientAttribution>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("client"))
        .collect::<Vec<_>>();
    if tags.is_empty() {
        return Ok(None);
    }
    if tags.len() != 1 {
        return Err(invalid("an event has one client tag"));
    }
    let values = tags[0].as_slice();
    if values.len() < 3 || values.len() > 4 || values[1].is_empty() || values[1].len() > 256 {
        return Err(invalid(
            "a client tag names the application, a 31990 address, and an optional relay",
        ));
    }
    let handler = handler_address(&values[2])?;
    let relay = match values.get(3) {
        None => None,
        Some(value) => optional_relay(value)?,
    };
    Ok(Some(ClientAttribution {
        name: values[1].clone(),
        handler,
        relay,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::domain::hex::decode_lower_hex;
    use crate::domain::{
        EventClass, Filter, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };
    use crate::nip19;

    fn user() -> RelaySigner {
        RelaySigner::from_secret_hex(&"89".repeat(32)).unwrap()
    }

    fn app() -> RelaySigner {
        RelaySigner::from_secret_hex(&"90".repeat(32)).unwrap()
    }

    #[test]
    fn a_recommendation_points_at_a_handler_and_the_url_receives_the_entity() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/89.md"
        ))
        .unwrap();
        assert!(text.contains("kind:31989"));
        assert!(text.contains("kind:31990"));
        assert!(text.contains("31337"));
        assert!(text.contains("bech32"));
        assert!(text.contains("1743058db7078661b94aaf4286429d97ee5257d14a86d6bfa54cb0482b876fb0"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "89.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "89.md")
        );

        let app = app();
        let user = user();
        let address = format!("31990:{}:abcd", app.pubkey());
        let manifest = format!("35128:{}:site", app.pubkey());
        let handler = app.sign(
            1_700_000_000,
            HANDLER_KIND,
            vec![
                Tag::new(vec!["d".into(), "abcd".into()]),
                Tag::new(vec!["k".into(), "31337".into()]),
                Tag::new(vec![
                    "latest".into(),
                    manifest,
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "web".into(),
                    "https://app.example/e/bech32".into(),
                    "nevent".into(),
                ]),
                Tag::new(vec!["ios".into(), "app://open/bech32".into()]),
            ],
            r#"{"name":"Zapstr"}"#.into(),
        );
        handler.validate_structure().unwrap();
        assert_eq!(handler.class(), EventClass::Addressable);
        let opened = open_handler(&handler).unwrap();
        assert_eq!(opened.identifier, "abcd");
        assert_eq!(opened.kinds, vec![31_337]);
        assert_eq!(
            opened
                .metadata
                .as_ref()
                .and_then(|map| map.get("name"))
                .and_then(|value| value.as_str()),
            Some("Zapstr")
        );
        assert_eq!(
            opened
                .latest
                .as_ref()
                .map(|item| item.address.identifier.as_str()),
            Some("site")
        );
        let web = link_for(&opened, "web", Some("nevent")).unwrap();
        let npub = nip19::encode_npub(&decode_lower_hex::<32>(user.pubkey(), "user").unwrap());
        let url = handler_url(&web.url, &npub).unwrap();
        assert!(url.contains(&npub));
        assert!(!url.contains("bech32"));
        assert!(
            link_for(&opened, "ios", Some("nevent"))
                .unwrap()
                .entity
                .is_none()
        );

        let recommendation = user.sign(
            1_700_000_000,
            RECOMMENDATION_KIND,
            vec![
                Tag::new(vec!["d".into(), "31337".into()]),
                Tag::new(vec![
                    "a".into(),
                    address,
                    "wss://relay.example".into(),
                    "web".into(),
                ]),
            ],
            String::new(),
        );
        recommendation.validate_structure().unwrap();
        assert_eq!(recommendation.class(), EventClass::Addressable);
        let recommended = open_recommendation(&recommendation).unwrap();
        assert_eq!(recommended.kind, 31_337);
        assert_eq!(recommended.handlers[0].platform.as_deref(), Some("web"));
        assert_eq!(recommended.handlers[0].handler.pubkey, app.pubkey());
        let mut recommendation_tags = BTreeMap::new();
        recommendation_tags.insert("d".into(), vec!["31337".into()]);
        let filter = Filter {
            kinds: Some(vec![RECOMMENDATION_KIND]),
            tags: recommendation_tags,
            authors: Some(vec![user.pubkey().to_owned()]),
            ..Filter::default()
        };
        assert!(filter.matches(&recommendation));
        let mut handler_tags = BTreeMap::new();
        handler_tags.insert("k".into(), vec!["31337".into()]);
        let handlers = Filter {
            kinds: Some(vec![HANDLER_KIND]),
            tags: handler_tags,
            ..Filter::default()
        };
        assert!(handlers.matches(&handler));

        let newer = user.sign(
            1_700_000_100,
            RECOMMENDATION_KIND,
            vec![
                Tag::new(vec!["d".into(), "31337".into()]),
                Tag::new(vec!["a".into(), format!("31990:{}:abcd", app.pubkey())]),
            ],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&recommendation, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let note = user.sign(
            1_700_000_200,
            1,
            vec![Tag::new(vec![
                "client".into(),
                "My Client".into(),
                format!("31990:{}:abcd", app.pubkey()),
                "wss://relay.example".into(),
            ])],
            "hello".into(),
        );
        note.validate_structure().unwrap();
        let client = open_client_tag(&note).unwrap().unwrap();
        assert_eq!(client.name, "My Client");
        assert_eq!(client.handler.identifier, "abcd");
        let opted_out = user.sign(1_700_000_300, 1, Vec::new(), "hello".into());
        opted_out.validate_structure().unwrap();
        assert!(open_client_tag(&opted_out).unwrap().is_none());

        let missing = user.sign(
            1_700_000_400,
            RECOMMENDATION_KIND,
            vec![Tag::new(vec!["d".into(), "31337".into()])],
            String::new(),
        );
        assert!(missing.validate_structure().is_err());
        let bare_url = app.sign(
            1_700_000_500,
            HANDLER_KIND,
            vec![
                Tag::new(vec!["d".into(), "abcd".into()]),
                Tag::new(vec!["k".into(), "31337".into()]),
                Tag::new(vec![
                    "web".into(),
                    "https://app.example/e/".into(),
                    "nevent".into(),
                ]),
            ],
            String::new(),
        );
        assert!(bare_url.validate_structure().is_err());
    }
}
