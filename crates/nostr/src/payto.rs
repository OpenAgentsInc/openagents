//! Where a Nostr key says it is paid: the NIP-A3 payment targets (kind
//! 10133, `["payto","spark","spark1…"]`) and the kind-0 profile's `lud16`,
//! with the shape rules for each address.
//!
//! Shared by the phone's Wallet (`crates/openagents-mobile/src/payees.rs`)
//! and the pay ledger's payout destination resolver
//! (`crates/pay-ledger/src/payee.rs`). Only events signed by the key count,
//! and the newest valid event of each kind wins.

use serde_json::{Map, Value};

use crate::domain::{Event, open_payment_targets};

/// NIP-A3 payment targets.
pub const PAYMENT_TARGETS_KIND: u16 = 10_133;
/// The `payto` type the phone publishes and prefers.
pub const SPARK_TYPE: &str = "spark";

/// `name@domain`, lower case, when it has that shape.
#[must_use]
pub fn lightning_address(text: &str) -> Option<String> {
    let text = text.trim().to_ascii_lowercase();
    let (name, domain) = text.split_once('@')?;
    let name_ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'));
    let domain_ok = domain.contains('.')
        && domain.len() <= 253
        && !domain.starts_with(['.', '-'])
        && !domain.ends_with(['.', '-'])
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    (name_ok && domain_ok).then_some(text)
}

/// A mainnet Spark address, when it has that shape. Regtest and testnet
/// prefixes (`sparkrt1`, `sparkt1`, ...) are refused.
#[must_use]
pub fn spark_address(text: &str) -> Option<String> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("spark1")
        && (40..=200).contains(&lower.len())
        && lower.chars().all(|c| c.is_ascii_alphanumeric())
        && (text == lower || text == text.to_ascii_uppercase()))
    .then_some(lower)
}

/// The newest event of `kind` signed by `pubkey` (hex), ignoring events by
/// another key or with a bad ID or signature.
#[must_use]
pub fn newest_signed<'a>(pubkey: &str, kind: u16, events: &'a [Event]) -> Option<&'a Event> {
    events
        .iter()
        .filter(|event| {
            event.kind == kind && event.pubkey == pubkey && event.validate_crypto().is_ok()
        })
        .max_by_key(|event| (event.created_at, event.id.clone()))
}

/// What a key published about being paid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Published {
    /// The newest valid kind-0 profile's fields, for callers that read more
    /// (the phone reads the display name).
    pub metadata: Option<Map<String, Value>>,
    /// `lud16` from that profile, when it has the shape.
    pub lightning_address: Option<String>,
    /// The first mainnet Spark `payto` target of the newest valid kind 10133.
    pub spark: Option<String>,
    /// When the profile and the targets were signed.
    pub profile_at: Option<u64>,
    pub targets_at: Option<u64>,
}

/// Read what `pubkey` (hex) published from events a relay returned.
#[must_use]
pub fn published(pubkey: &str, events: &[Event]) -> Published {
    let mut out = Published::default();
    if let Some(metadata) = newest_signed(pubkey, 0, events)
        && let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&metadata.content)
    {
        out.lightning_address = fields
            .get("lud16")
            .and_then(Value::as_str)
            .and_then(lightning_address);
        out.profile_at = Some(metadata.created_at);
        out.metadata = Some(fields);
    }
    if let Some(targets) = newest_signed(pubkey, PAYMENT_TARGETS_KIND, events)
        && let Ok(list) = open_payment_targets(targets)
    {
        out.spark = list
            .iter()
            .filter(|target| target.payment_type == SPARK_TYPE)
            .find_map(|target| spark_address(&target.address));
        out.targets_at = Some(targets.created_at);
    }
    out
}

/// The payout a signed EXT release names, when `event` is a valid release
/// signed by `pubkey` (hex). The release's own checks refuse a regtest
/// Spark address or a malformed one.
#[must_use]
pub fn release_payout(pubkey: &str, event: &Event) -> Option<String> {
    if event.pubkey != pubkey || event.kind != crate::ext::RELEASE_KIND {
        return None;
    }
    let value = crate::ext::parse_record(event).ok()?;
    value.get("payout")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn shapes() {
        assert_eq!(
            lightning_address(" Alice@Example.com "),
            Some("alice@example.com".into())
        );
        assert!(lightning_address("alice@localhost").is_none());
        let spark = format!("spark1{}", "q".repeat(60));
        assert_eq!(spark_address(&spark), Some(spark.clone()));
        assert!(spark_address(&format!("sparkrt1{}", "q".repeat(60))).is_none());
        assert!(spark_address("spark1short").is_none());
    }

    #[test]
    fn only_the_keys_newest_signed_events_count() {
        let owner = signer("11");
        let other = signer("22");
        let spark = format!("spark1{}", "q".repeat(60));
        let tags = |address: &str| {
            vec![Tag::new(vec![
                "payto".into(),
                "spark".into(),
                address.into(),
            ])]
        };
        let mut forged = owner.sign(30, PAYMENT_TARGETS_KIND, tags(&spark), String::new());
        forged.content = "changed".into();
        let events = vec![
            owner.sign(10, 0, vec![], r#"{"lud16":"old@example.com"}"#.into()),
            owner.sign(20, 0, vec![], r#"{"lud16":"new@example.com"}"#.into()),
            other.sign(40, 0, vec![], r#"{"lud16":"thief@example.com"}"#.into()),
            owner.sign(5, PAYMENT_TARGETS_KIND, tags(&spark), String::new()),
            forged,
        ];
        let found = published(owner.pubkey(), &events);
        assert_eq!(found.lightning_address.as_deref(), Some("new@example.com"));
        assert_eq!(found.spark.as_deref(), Some(spark.as_str()));
        assert_eq!(found.targets_at, Some(5));
        assert_eq!(
            published(other.pubkey(), &events[3..4]),
            Published::default()
        );
    }
}
