//! NIP-85 trusted assertions.
//!
//! Kinds `30382`, `30383`, `30384`, and `30385` are addressable scores
//! for a pubkey, an event, an address, or a NIP-73 identifier. `rank`
//! is an integer from 0 to 100. The other declared counts are
//! non-negative integers. Kind `10040` lists the providers a person
//! trusts. Public rows are tags. Private rows are a NIP-44 ciphertext
//! of the same JSON.
//!
//! The relay does not compute the scores, read a kind `0` profile, or
//! load a provider website. Admission does not decrypt a provider list.
//! NIP-85 is a draft, so these kinds stay off the NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, EventClass, ReplacementAddress};

const PROVIDER_KIND: u16 = 10_040;
const USER_KIND: u16 = 30_382;
const EVENT_KIND: u16 = 30_383;
const ADDRESS_KIND: u16 = 30_384;
const EXTERNAL_KIND: u16 = 30_385;

const USER_COUNTS: &[&str] = &[
    "followers",
    "post_cnt",
    "reply_cnt",
    "reactions_cnt",
    "zap_amt_recd",
    "zap_amt_sent",
    "zap_cnt_recd",
    "zap_cnt_sent",
    "zap_avg_amt_day_recd",
    "zap_avg_amt_day_sent",
    "reports_cnt_recd",
    "reports_cnt_sent",
];
const EVENT_COUNTS: &[&str] = &[
    "comment_cnt",
    "quote_cnt",
    "repost_cnt",
    "reaction_cnt",
    "zap_cnt",
    "zap_amount",
];

/// A numeric score or a topic string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricValue {
    Count(u64),
    Topic(String),
}

/// One result on a trusted assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
}

/// A kind `30382` through `30385` assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedAssertion {
    pub kind: u16,
    pub subject: String,
    pub metrics: Vec<Metric>,
}

/// One provider row: assertion kind, metric, service key, and relay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderSource {
    pub assertion_kind: u16,
    pub metric: String,
    pub service: String,
    pub relay: String,
}

/// The public rows of a kind `10040` provider list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedProviders {
    pub sources: Vec<ProviderSource>,
    pub private_list: bool,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn whole(value: &str, max: Option<u64>, reason: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    let parsed = value.parse::<u64>().map_err(|_| invalid(reason))?;
    if max.is_some_and(|max| parsed > max) {
        return Err(invalid(reason));
    }
    Ok(parsed)
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "assertion subject")
        .map_err(|_| invalid("an assertion subject is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn relay_url(value: &str) -> bool {
    (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn known_metric(kind: u16, name: &str) -> bool {
    match kind {
        USER_KIND => {
            name == "rank"
                || name == "first_created_at"
                || name == "t"
                || name == "active_hours_start"
                || name == "active_hours_end"
                || USER_COUNTS.contains(&name)
        }
        EVENT_KIND | ADDRESS_KIND => name == "rank" || EVENT_COUNTS.contains(&name),
        EXTERNAL_KIND => matches!(name, "rank" | "comment_cnt" | "reaction_cnt"),
        _ => false,
    }
}

fn metric_value(kind: u16, name: &str, value: &str) -> Result<MetricValue, DomainError> {
    match name {
        "rank" => Ok(MetricValue::Count(whole(
            value,
            Some(100),
            "a rank is an integer from 0 to 100",
        )?)),
        "active_hours_start" | "active_hours_end" => Ok(MetricValue::Count(whole(
            value,
            Some(24),
            "an active hour is an integer from 0 to 24",
        )?)),
        "t" if kind == USER_KIND => {
            if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
                return Err(invalid("a topic is 1 to 64 characters"));
            }
            Ok(MetricValue::Topic(value.to_owned()))
        }
        "first_created_at" => Ok(MetricValue::Count(whole(
            value,
            None,
            "a first post time is unix seconds",
        )?)),
        _ if known_metric(kind, name) => Ok(MetricValue::Count(whole(
            value,
            None,
            "an assertion count is a non-negative integer",
        )?)),
        _ => Err(invalid("an assertion metric is one of the declared names")),
    }
}

fn external_kind(value: &str) -> Result<&'static str, DomainError> {
    if let Some(topic) = value.strip_prefix('#')
        && !topic.is_empty()
        && topic
            .chars()
            .all(|char| !char.is_ascii_uppercase() && !char.is_whitespace())
    {
        return Ok("#");
    }
    if (value.starts_with("https://") || value.starts_with("http://"))
        && !value.contains('#')
        && !value.chars().any(char::is_whitespace)
    {
        return Ok("web");
    }
    if let Some(id) = value.strip_prefix("isbn:") {
        let digits = id.chars().all(|char| char.is_ascii_digit());
        if digits && (id.len() == 10 || id.len() == 13) {
            return Ok("isbn");
        }
    }
    if let Some(id) = value.strip_prefix("geo:")
        && !id.is_empty()
        && id
            .chars()
            .all(|char| "0123456789bcdefghjkmnpqrstuvwxyz".contains(char))
    {
        return Ok("geo");
    }
    Err(invalid(
        "an external subject is a hashtag, web URL, isbn, or geohash",
    ))
}

fn subject(kind: u16, value: &str) -> Result<String, DomainError> {
    match kind {
        USER_KIND | EVENT_KIND => pubkey(value),
        ADDRESS_KIND => {
            let address = ReplacementAddress::from_str(value)
                .map_err(|_| invalid("an address subject is an addressable event"))?;
            if EventClass::from_kind(address.kind) != EventClass::Addressable {
                return Err(invalid("an address subject is an addressable event"));
            }
            Ok(value.to_owned())
        }
        EXTERNAL_KIND => {
            external_kind(value)?;
            Ok(value.to_owned())
        }
        _ => Err(invalid(
            "a trusted assertion has kind 30382, 30383, 30384, or 30385",
        )),
    }
}

fn hint_matches(event: &Event, kind: u16, subject: &str) -> Result<(), DomainError> {
    let (name, expected) = match kind {
        USER_KIND => ("p", subject),
        EVENT_KIND => ("e", subject),
        ADDRESS_KIND => ("a", subject),
        _ => return Ok(()),
    };
    for tag in event.tags.iter().filter(|tag| tag.name() == Some(name)) {
        let Some(value) = tag.value() else {
            return Err(invalid("a subject hint matches the d tag"));
        };
        if value != expected {
            return Err(invalid("a subject hint matches the d tag"));
        }
        if let Some(relay) = tag.as_slice().get(2)
            && !relay.is_empty()
            && !relay_url(relay)
        {
            return Err(invalid("a subject hint relay is ws:// or wss://"));
        }
    }
    Ok(())
}

/// Read a kind `30382`, `30383`, `30384`, or `30385` assertion.
pub fn open_trusted_assertion(event: &Event) -> Result<TrustedAssertion, DomainError> {
    if !matches!(
        event.kind,
        USER_KIND | EVENT_KIND | ADDRESS_KIND | EXTERNAL_KIND
    ) {
        return Err(invalid(
            "a trusted assertion has kind 30382, 30383, 30384, or 30385",
        ));
    }
    let identifiers: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect();
    if identifiers.len() != 1 {
        return Err(invalid("a trusted assertion has one subject"));
    }
    let Some(raw) = identifiers[0].value() else {
        return Err(invalid("a trusted assertion has one subject"));
    };
    let subject = subject(event.kind, raw)?;
    if event.kind == EXTERNAL_KIND {
        let kinds: Vec<_> = event.tag_values("k").collect();
        if kinds.len() != 1 || kinds[0] != external_kind(&subject)? {
            return Err(invalid("an external assertion k tag matches the subject"));
        }
    }
    hint_matches(event, event.kind, &subject)?;
    let mut metrics = Vec::new();
    for tag in &event.tags {
        let Some(name) = tag.name() else {
            continue;
        };
        if !known_metric(event.kind, name) {
            continue;
        }
        if name != "t" && metrics.iter().any(|metric: &Metric| metric.name == name) {
            return Err(invalid("an assertion metric is listed once"));
        }
        let Some(value) = tag.value() else {
            return Err(invalid("an assertion metric has a value"));
        };
        let parsed = metric_value(event.kind, name, value)?;
        if name == "t"
            && metrics
                .iter()
                .any(|metric| metric.name == "t" && metric.value == parsed)
        {
            return Err(invalid("a topic is listed once"));
        }
        metrics.push(Metric {
            name: name.to_owned(),
            value: parsed,
        });
    }
    if metrics.is_empty() {
        return Err(invalid("a trusted assertion has one metric"));
    }
    Ok(TrustedAssertion {
        kind: event.kind,
        subject,
        metrics,
    })
}

fn provider_source(name: &str, service: &str, relay: &str) -> Result<ProviderSource, DomainError> {
    let Some((kind, metric)) = name.split_once(':') else {
        return Err(invalid("a provider row names a kind and a metric"));
    };
    let assertion_kind = kind
        .parse::<u16>()
        .map_err(|_| invalid("a provider row names a kind and a metric"))?;
    if !known_metric(assertion_kind, metric) {
        return Err(invalid("a provider row names a kind and a metric"));
    }
    pubkey(service)?;
    if !relay_url(relay) {
        return Err(invalid("a provider relay is ws:// or wss://"));
    }
    Ok(ProviderSource {
        assertion_kind,
        metric: metric.to_owned(),
        service: service.to_owned(),
        relay: relay.to_owned(),
    })
}

/// Read the decrypted JSON array of provider rows.
pub fn parse_provider_list(plaintext: &str) -> Result<Vec<ProviderSource>, DomainError> {
    let value: serde_json::Value =
        serde_json::from_str(plaintext).map_err(|_| invalid("a provider list is a JSON array"))?;
    let Some(rows) = value.as_array() else {
        return Err(invalid("a provider list is a JSON array"));
    };
    if rows.is_empty() {
        return Err(invalid("a provider list names a source"));
    }
    rows.iter()
        .map(|row| {
            let Some(fields) = row.as_array() else {
                return Err(invalid("a provider row has a kind, a key, and a relay"));
            };
            if fields.len() != 3 {
                return Err(invalid("a provider row has a kind, a key, and a relay"));
            }
            let [name, service, relay] = fields
                .iter()
                .map(|field| {
                    field
                        .as_str()
                        .ok_or_else(|| invalid("a provider row has a kind, a key, and a relay"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| invalid("a provider row has a kind, a key, and a relay"))?;
            provider_source(name, service, relay)
        })
        .collect()
}

/// Read the public rows of a kind `10040` provider list.
pub fn open_trusted_providers(event: &Event) -> Result<TrustedProviders, DomainError> {
    if event.kind != PROVIDER_KIND {
        return Err(invalid("a provider list has kind 10040"));
    }
    let private_list = if event.content.is_empty() {
        false
    } else {
        crate::nip44::payload_shape(&event.content)
            .map_err(|_| invalid("a private provider list is NIP-44 ciphertext"))?;
        true
    };
    let mut sources = Vec::new();
    for tag in &event.tags {
        let Some(name) = tag.name() else {
            return Err(invalid("a provider row names a kind and a metric"));
        };
        let parts = tag.as_slice();
        if parts.len() != 3 {
            return Err(invalid("a provider row has a kind, a key, and a relay"));
        }
        let source = provider_source(name, &parts[1], &parts[2])?;
        if sources.contains(&source) {
            return Err(invalid("a provider row is listed once"));
        }
        sources.push(source);
    }
    if sources.is_empty() && !private_list {
        return Err(invalid("a provider list names a source"));
    }
    Ok(TrustedProviders {
        sources,
        private_list,
    })
}

#[cfg(test)]
mod tests {
    use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

    use super::decode_lower_hex;
    use super::*;
    use crate::domain::{RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn secret(byte: &str) -> SecretKey {
        SecretKey::from_byte_array(decode_lower_hex::<32>(&byte.repeat(32), "key").unwrap())
            .unwrap()
    }

    fn xonly(secret: &SecretKey) -> XOnlyPublicKey {
        Keypair::from_secret_key(&Secp256k1::signing_only(), secret)
            .x_only_public_key()
            .0
    }

    #[test]
    fn a_trusted_rank_replaces_and_a_provider_list_keeps_a_private_source() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/85.md"
        ))
        .unwrap();
        assert!(text.contains("30382"));
        assert!(text.contains("10040"));
        assert!(text.contains("rank"));
        assert!(text.contains("30385"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "85.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "85.md")
        );

        let service = signer("85");
        let user = signer("86");
        let relay = "wss://nip85.example";
        let ranked = service.sign(
            1_700_000_000,
            USER_KIND,
            vec![
                Tag::new(vec!["d".into(), user.pubkey().to_owned()]),
                Tag::new(vec!["rank".into(), "89".into()]),
                Tag::new(vec!["followers".into(), "12".into()]),
                Tag::new(vec!["p".into(), user.pubkey().to_owned(), relay.into()]),
            ],
            String::new(),
        );
        ranked.validate_structure().unwrap();
        assert_eq!(ranked.class(), EventClass::Addressable);
        let assertion = open_trusted_assertion(&ranked).unwrap();
        assert_eq!(assertion.subject, user.pubkey());
        assert!(
            assertion
                .metrics
                .iter()
                .any(|metric| { metric.name == "rank" && metric.value == MetricValue::Count(89) })
        );
        assert!(assertion.metrics.iter().any(|metric| {
            metric.name == "followers" && metric.value == MetricValue::Count(12)
        }));

        let revised = service.sign(
            1_700_000_100,
            USER_KIND,
            vec![
                Tag::new(vec!["d".into(), user.pubkey().to_owned()]),
                Tag::new(vec!["rank".into(), "90".into()]),
            ],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&ranked, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        let over = service.sign(
            1_700_000_150,
            USER_KIND,
            vec![
                Tag::new(vec!["d".into(), user.pubkey().to_owned()]),
                Tag::new(vec!["rank".into(), "101".into()]),
            ],
            String::new(),
        );
        assert!(over.validate_structure().is_err());

        let event_id = "ab".repeat(32);
        let event_rank = service.sign(
            1_700_000_200,
            EVENT_KIND,
            vec![
                Tag::new(vec!["d".into(), event_id.clone()]),
                Tag::new(vec!["rank".into(), "50".into()]),
                Tag::new(vec!["comment_cnt".into(), "3".into()]),
            ],
            String::new(),
        );
        event_rank.validate_structure().unwrap();
        assert_eq!(
            open_trusted_assertion(&event_rank).unwrap().subject,
            event_id
        );

        let address = format!("34550:{}:gardening", user.pubkey());
        let address_rank = service.sign(
            1_700_000_250,
            ADDRESS_KIND,
            vec![
                Tag::new(vec!["d".into(), address.clone()]),
                Tag::new(vec!["rank".into(), "70".into()]),
            ],
            String::new(),
        );
        address_rank.validate_structure().unwrap();
        assert_eq!(
            open_trusted_assertion(&address_rank).unwrap().subject,
            address
        );

        let topic = service.sign(
            1_700_000_300,
            EXTERNAL_KIND,
            vec![
                Tag::new(vec!["d".into(), "#nostr".into()]),
                Tag::new(vec!["k".into(), "#".into()]),
                Tag::new(vec!["rank".into(), "10".into()]),
            ],
            String::new(),
        );
        topic.validate_structure().unwrap();
        let external = open_trusted_assertion(&topic).unwrap();
        assert_eq!(external.subject, "#nostr");
        assert_eq!(external.metrics[0].value, MetricValue::Count(10));

        let user_secret = secret("86");
        let private = format!(r#"[["30383:rank","{}","{relay}"]]"#, service.pubkey());
        let key = crate::nip44::conversation_key(&user_secret, &xonly(&user_secret));
        let sealed = crate::nip44::encrypt(&private, &key, [0x85; 32]).unwrap();
        let providers = user.sign(
            1_700_000_400,
            PROVIDER_KIND,
            vec![Tag::new(vec![
                "30382:rank".into(),
                service.pubkey().to_owned(),
                relay.into(),
            ])],
            sealed,
        );
        providers.validate_structure().unwrap();
        assert_eq!(providers.class(), EventClass::Replaceable);
        let listed = open_trusted_providers(&providers).unwrap();
        assert!(listed.private_list);
        assert_eq!(listed.sources.len(), 1);
        assert_eq!(listed.sources[0].assertion_kind, USER_KIND);
        assert_eq!(listed.sources[0].metric, "rank");
        assert_eq!(listed.sources[0].service, service.pubkey());
        let plaintext = crate::nip44::decrypt(&providers.content, &key).unwrap();
        let hidden = parse_provider_list(&plaintext).unwrap();
        assert_eq!(hidden[0].assertion_kind, EVENT_KIND);
        assert_eq!(hidden[0].metric, "rank");
        assert_eq!(hidden[0].relay, relay);

        let replaced = user.sign(
            1_700_000_500,
            PROVIDER_KIND,
            vec![Tag::new(vec![
                "30382:followers".into(),
                service.pubkey().to_owned(),
                relay.into(),
            ])],
            String::new(),
        );
        replaced.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&providers, &replaced),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        assert!(!open_trusted_providers(&replaced).unwrap().private_list);
    }
}
