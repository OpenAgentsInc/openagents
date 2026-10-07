//! Where a payee's payouts go: the payout destination resolver.
//!
//! Design: `docs/payments/2026-10-02-central-receive-and-splits.md`,
//! section 5. First match wins:
//!
//! 1. The signed EXT release's `payout` (a Spark address, a Lightning
//!    address, or a node key), pinned with the release; for the owner of a
//!    hosted resource, the `payout` of their signed registration (#10194).
//! 2. The newest signed NIP-A3 payment targets (kind 10133) Spark target.
//! 3. `lud16` in the newest signed kind-0 profile.
//! 4. An API account's payout setting (`PUT /v1/account/payout`).
//! 5. None: shares accrue and wait.
//!
//! Only events signed by the payee's key count, and only mainnet shapes
//! (`nostr::payto`, the rules the phone's Wallet uses). A result is cached
//! in the `payee` table with its `source` and `verified_at`; a party is
//! re-resolved at most once an hour. A cached destination stays until a
//! newer resolution replaces it.

use nostr::domain::Event;
use nostr::payto;
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::{Ledger, Payee, Result};

/// How long a resolution (found or not) is trusted before re-resolving.
pub const RECHECK_SECS: i64 = 3600;

/// What kind of address a destination is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Spark,
    LightningAddress,
    NodeKey,
}

impl Kind {
    /// The `payee.destination_kind` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spark => "spark",
            Self::LightningAddress => "lud16",
            Self::NodeKey => "node",
        }
    }
}

/// Where a destination came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Release,
    /// A signed hosted resource registration's `payout`.
    Registration,
    PaymentTargets,
    Profile,
    Account,
}

impl Source {
    /// The `payee.source` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Registration => "registration",
            Self::PaymentTargets => "nip-a3",
            Self::Profile => "profile",
            Self::Account => "account",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub kind: Kind,
    pub value: String,
    pub source: Source,
}

/// What is known about a party, gathered by the caller (relay reads, the
/// pinned release, the account store).
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// The party's Nostr key (hex), when it is a Nostr identity.
    pub pubkey: Option<String>,
    /// The signed EXT release the shares were earned under.
    pub release: Option<Event>,
    /// The `payout` of the party's newest verified hosted resource
    /// registration (the pay front checks its signature).
    pub registration: Option<String>,
    /// Kind-0 and kind-10133 events a relay returned for the key.
    pub events: Vec<Event>,
    /// The API account's payout setting.
    pub account_payout: Option<String>,
}

/// The kind of `text` as a payout address, when it is a mainnet one.
#[must_use]
pub fn classify(text: &str) -> Option<(Kind, String)> {
    if let Some(spark) = payto::spark_address(text) {
        return Some((Kind::Spark, spark));
    }
    if let Some(address) = payto::lightning_address(text) {
        return Some((Kind::LightningAddress, address));
    }
    let node = text.len() == 66
        && (text.starts_with("02") || text.starts_with("03"))
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    node.then(|| (Kind::NodeKey, text.to_owned()))
}

/// Resolve a destination from `sources`, first match wins.
#[must_use]
pub fn resolve(sources: &Sources) -> Option<Destination> {
    let found = |source: Source, text: Option<String>| {
        text.and_then(|text| classify(&text))
            .map(|(kind, value)| Destination {
                kind,
                value,
                source,
            })
    };
    let pubkey = sources.pubkey.as_deref();
    let release = pubkey
        .zip(sources.release.as_ref())
        .and_then(|(key, event)| payto::release_payout(key, event));
    let published = pubkey.map(|key| payto::published(key, &sources.events));
    found(Source::Release, release)
        .or_else(|| found(Source::Registration, sources.registration.clone()))
        .or_else(|| {
            found(
                Source::PaymentTargets,
                published.as_ref().and_then(|p| p.spark.clone()),
            )
        })
        .or_else(|| {
            found(
                Source::Profile,
                published.as_ref().and_then(|p| p.lightning_address.clone()),
            )
        })
        .or_else(|| {
            // An account setting is a Spark or Lightning address only.
            found(Source::Account, sources.account_payout.clone())
                .filter(|d| d.kind != Kind::NodeKey)
        })
}

const TABLES: &str = "CREATE TABLE IF NOT EXISTS payee_check (
    party TEXT PRIMARY KEY,
    checked_at INTEGER NOT NULL
);";

impl Ledger {
    /// The cached destination for `party`.
    pub fn payee(&self, party: &str) -> Result<Option<Payee>> {
        Ok(self
            .connection
            .query_row(
                "SELECT party,destination_kind,destination_value,source,verified_at FROM payee WHERE party=?",
                [party],
                |r| {
                    Ok(Payee {
                        party: r.get(0)?,
                        destination_kind: r.get(1)?,
                        destination_value: r.get(2)?,
                        source: r.get(3)?,
                        verified_at: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// The destination for `party` at `now` (Unix seconds): the cached one
    /// when it was resolved within [`RECHECK_SECS`], otherwise resolved
    /// again from `sources()` (called only then) and cached.
    pub fn resolve_payee(
        &mut self,
        party: &str,
        now: i64,
        sources: impl FnOnce() -> Sources,
    ) -> Result<Option<Payee>> {
        self.connection.execute_batch(TABLES)?;
        let checked: Option<i64> = self
            .connection
            .query_row(
                "SELECT checked_at FROM payee_check WHERE party=?",
                [party],
                |r| r.get(0),
            )
            .optional()?;
        if checked.is_some_and(|at| now - at < RECHECK_SECS && at <= now) {
            return self.payee(party);
        }
        let mut sources = sources();
        // Relay reads happen before the write lock. Select the canonical
        // account fallback and cache the result in one transaction, so a
        // concurrent destination update cannot be overwritten by stale data.
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let setting: Option<String> = tx
            .query_row(
                "SELECT value FROM account_payout WHERE party=?",
                [party],
                |r| r.get(0),
            )
            .optional()?;
        sources.account_payout = setting.or(sources.account_payout);
        if let Some(found) = resolve(&sources) {
            crate::register_payee_in(
                &tx,
                Payee {
                    party: party.to_owned(),
                    destination_kind: found.kind.as_str().into(),
                    destination_value: found.value,
                    source: found.source.as_str().into(),
                    verified_at: now,
                },
            )?;
        }
        tx.execute(
            "INSERT INTO payee_check(party,checked_at) VALUES(?,?) ON CONFLICT(party) DO UPDATE SET checked_at=excluded.checked_at",
            params![party, now],
        )?;
        tx.commit()?;
        self.payee(party)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::{RelaySigner, Tag};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn spark(prefix: &str) -> String {
        format!("{prefix}1{}", "q".repeat(60))
    }

    fn release(by: &RelaySigner, payout: Value) -> Event {
        let bytes = br#"{"type":"object"}"#;
        let digest: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut body = json!({
            "v": 1, "requires": [], "type": "release",
            "package": format!("{}:demo", by.pubkey()), "version": "1.0.0",
            "manifest": {"digest": format!("sha256:{digest}"), "size": bytes.len(), "media_type": "application/json"},
            "fee_msat": 10_000,
        });
        body["payout"] = payout;
        by.sign(
            1_700_000_000,
            nostr::ext::RELEASE_KIND,
            vec![Tag::new(vec!["t".into(), "oa:ext:release:v1".into()])],
            body.to_string(),
        )
    }

    fn profile(by: &RelaySigner, at: u64, lud16: &str) -> Event {
        by.sign(at, 0, vec![], json!({ "lud16": lud16 }).to_string())
    }

    fn targets(by: &RelaySigner, at: u64, address: &str) -> Event {
        by.sign(
            at,
            payto::PAYMENT_TARGETS_KIND,
            vec![Tag::new(vec![
                "payto".into(),
                "spark".into(),
                address.into(),
            ])],
            String::new(),
        )
    }

    fn full(author: &RelaySigner) -> Sources {
        Sources {
            pubkey: Some(author.pubkey().to_owned()),
            release: Some(release(author, json!("alice@getalby.com"))),
            registration: Some("dave@example.com".into()),
            events: vec![
                targets(author, 10, &spark("spark")),
                profile(author, 10, "bob@example.com"),
            ],
            account_payout: Some("carol@example.com".into()),
        }
    }

    fn source(sources: &Sources) -> Option<(Source, String)> {
        resolve(sources).map(|d| (d.source, d.value))
    }

    #[test]
    fn each_source_wins_in_order() {
        let author = signer("11");
        let mut sources = full(&author);
        assert_eq!(
            source(&sources),
            Some((Source::Release, "alice@getalby.com".into()))
        );
        sources.release = None;
        assert_eq!(
            source(&sources),
            Some((Source::Registration, "dave@example.com".into()))
        );
        sources.registration = None;
        assert_eq!(
            source(&sources),
            Some((Source::PaymentTargets, spark("spark")))
        );
        sources.events.remove(0);
        assert_eq!(
            source(&sources),
            Some((Source::Profile, "bob@example.com".into()))
        );
        sources.events.clear();
        assert_eq!(
            source(&sources),
            Some((Source::Account, "carol@example.com".into()))
        );
        sources.account_payout = None;
        assert_eq!(source(&sources), None);
        // An account with no Nostr identity uses its setting.
        let account = Sources {
            account_payout: Some(spark("spark")),
            ..Sources::default()
        };
        assert_eq!(source(&account), Some((Source::Account, spark("spark"))));
    }

    #[test]
    fn a_release_node_key_payout_is_kept() {
        let author = signer("11");
        let key = format!("02{}", "ab".repeat(32));
        let sources = Sources {
            pubkey: Some(author.pubkey().to_owned()),
            release: Some(release(&author, json!(key))),
            ..Sources::default()
        };
        let found = resolve(&sources).unwrap();
        assert_eq!((found.kind, found.value), (Kind::NodeKey, key));
    }

    #[test]
    fn unsigned_and_wrong_key_events_are_ignored() {
        let author = signer("11");
        let thief = signer("22");
        // A release, targets, and profile signed by another key.
        let mut sources = Sources {
            pubkey: Some(author.pubkey().to_owned()),
            release: Some(release(&thief, json!("thief@example.com"))),
            registration: None,
            events: vec![
                targets(&thief, 50, &spark("spark")),
                profile(&thief, 50, "thief@example.com"),
            ],
            account_payout: None,
        };
        assert_eq!(source(&sources), None);
        // Tampered events by the right key: the signature no longer holds.
        let mut tampered = release(&author, json!("alice@getalby.com"));
        tampered.content = tampered.content.replace("alice", "mallory");
        let mut forged = profile(&author, 60, "bob@example.com");
        forged.content = json!({"lud16": "mallory@example.com"}).to_string();
        sources.release = Some(tampered);
        sources.events = vec![forged, profile(&author, 5, "bob@example.com")];
        assert_eq!(
            source(&sources),
            Some((Source::Profile, "bob@example.com".into()))
        );
        // The newest signed profile wins.
        sources
            .events
            .push(profile(&author, 70, "newer@example.com"));
        assert_eq!(
            source(&sources),
            Some((Source::Profile, "newer@example.com".into()))
        );
    }

    #[test]
    fn regtest_addresses_are_ignored() {
        let author = signer("11");
        let sources = Sources {
            pubkey: Some(author.pubkey().to_owned()),
            release: Some(release(&author, json!(spark("sparkrt")))),
            registration: Some(spark("sparkrt")),
            events: vec![
                targets(&author, 10, &spark("sparkrt")),
                profile(&author, 10, "bob@example.com"),
            ],
            account_payout: Some(spark("sparkrt")),
        };
        assert_eq!(
            source(&sources),
            Some((Source::Profile, "bob@example.com".into()))
        );
        let account = Sources {
            account_payout: Some(spark("sparkt")),
            ..Sources::default()
        };
        assert_eq!(source(&account), None);
        assert_eq!(classify("not an address"), None);
    }

    #[test]
    fn results_are_cached_with_source_and_rechecked_hourly() {
        let author = signer("11");
        let party = author.pubkey().to_owned();
        let mut ledger = Ledger::in_memory().unwrap();
        let payee = ledger
            .resolve_payee(&party, 1_000, || full(&author))
            .unwrap()
            .unwrap();
        assert_eq!(payee.source, "release");
        assert_eq!(payee.destination_kind, "lud16");
        assert_eq!(payee.verified_at, 1_000);
        // Within the hour the sources are not read again.
        let cached = ledger
            .resolve_payee(&party, 1_000 + RECHECK_SECS - 1, || {
                panic!("re-resolved within the hour")
            })
            .unwrap()
            .unwrap();
        assert_eq!(cached.destination_value, "alice@getalby.com");
        // After it, a newer source replaces the cache.
        let mut sources = full(&author);
        sources.release = None;
        sources.registration = None;
        let fresh = ledger
            .resolve_payee(&party, 1_000 + RECHECK_SECS, || sources)
            .unwrap()
            .unwrap();
        assert_eq!(fresh.source, "nip-a3");
        assert_eq!(fresh.destination_kind, "spark");
        assert_eq!(fresh.verified_at, 1_000 + RECHECK_SECS);
        // A party with nothing published is remembered as checked, too.
        assert!(
            ledger
                .resolve_payee("nobody", 5_000, Sources::default)
                .unwrap()
                .is_none()
        );
        assert!(
            ledger
                .resolve_payee("nobody", 5_001, || panic!("re-resolved"))
                .unwrap()
                .is_none()
        );
    }
}
