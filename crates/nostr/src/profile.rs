//! Relay admission for OpenAgents discovery profiles.
//!
//! Public CAP, PRG, EXT, and MKT records are parsed before storage. A valid
//! market discovery record does not prove its referenced capability or offering
//! is available. Private policy, RUN, and artifact content remains encrypted;
//! this layer cannot admit the application contract inside it. A relay `OK`
//! and an `EOSE` are not execution or completeness.

use serde_json::Value;

use crate::cap::{self, PREFERENCE_KIND};
use crate::contracts::{ContractError, RefusalCode, parse_strict};
use crate::domain::Event;
use crate::ext::{
    self, CHECKPOINT_KIND, LISTING_KIND, MIGRATION_KIND, RELEASE_KIND, REVOCATION_KIND,
};
use crate::market_contracts;
use crate::prg;
use crate::run::{self, HEAD_KIND, RECORD_KIND};

/// A relay acknowledgement does not mean the worker accepted or ran anything.
#[must_use]
pub const fn relay_ok_is_execution() -> bool {
    false
}

/// `EOSE` means the stored query ended. It does not mean the journal is complete.
#[must_use]
pub const fn eose_is_complete() -> bool {
    false
}

/// Whether publication of `event` requires NIP-42 authentication by its author.
#[must_use]
pub fn requires_author_auth(event: &Event) -> bool {
    event.kind == RECORD_KIND
        || event.kind == crate::contracts::ARTIFACT_ENVELOPE_KIND
        || event.kind == HEAD_KIND
        || (event.kind == PREFERENCE_KIND && marked(event, cap::PRIVATE_POLICY_MARKER))
}

/// Validate one OpenAgents profile event.
///
/// Kinds outside this set are unchanged.
///
/// # Errors
///
/// Returns a typed refusal when a profile event's kind, tags, or plaintext
/// body disagree.
pub fn admit(event: &Event) -> Result<(), ContractError> {
    match event.kind {
        cap::DISCOVERY_KIND => admit_definition(event, cap::CAP_MARKER, cap::parse_definition),
        PREFERENCE_KIND => admit_preference(event),
        prg::DISCOVERY_KIND => admit_definition(event, prg::PROGRAM_MARKER, prg::parse_definition),
        LISTING_KIND | RELEASE_KIND | REVOCATION_KIND | MIGRATION_KIND | CHECKPOINT_KIND => {
            ext::parse_record(event).map(|_| ())
        }
        RECORD_KIND | HEAD_KIND => run::admit_envelope(event),
        crate::contracts::ARTIFACT_ENVELOPE_KIND => crate::private_artifact::admit(event),
        market_contracts::OFFERING_KIND => market_contracts::parse_offering(event).map(|_| ()),
        market_contracts::HEAD_KIND => market_contracts::parse_head(event).map(|_| ()),
        _ => Ok(()),
    }
}

fn admit_preference(event: &Event) -> Result<(), ContractError> {
    if marked(event, cap::PRIVATE_POLICY_MARKER) {
        return cap::check_private_policy(event);
    }
    if !marked(event, cap::PUBLIC_POLICY_MARKER) {
        return Err(ContractError::new(
            RefusalCode::Malformed,
            "preference marker",
        ));
    }
    let value = plaintext(event)?;
    cap::parse_preference(&value).map(|_| ())
}

fn admit_definition<T>(
    event: &Event,
    marker: &str,
    parse: fn(&Value) -> Result<T, ContractError>,
) -> Result<(), ContractError> {
    if !marked(event, marker) || !has_identifier(event) {
        return Err(ContractError::new(RefusalCode::Malformed, "discovery tags"));
    }
    parse(&plaintext(event)?).map(|_| ())
}

fn plaintext(event: &Event) -> Result<Value, ContractError> {
    if event.content.trim_start().starts_with('{') {
        return parse_strict(event.content.as_bytes());
    }
    Err(ContractError::new(
        RefusalCode::ContentUnavailable,
        "profile content is not plaintext",
    ))
}

fn marked(event: &Event, marker: &str) -> bool {
    event
        .tag_values("t")
        .filter(|value| *value == marker)
        .count()
        == 1
}

fn has_identifier(event: &Event) -> bool {
    event
        .tag_values("d")
        .any(|value| !value.is_empty() && !value.contains('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};
    use serde_json::json;

    const PUB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SCHEMA: &str = "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0";

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap()
    }

    fn definition() -> Value {
        json!({
            "v": 1,
            "requires": [],
            "id": format!("{PUB}:openagents/demo"),
            "profile": "native",
            "summary": "a portable interface",
            "input": {"digest": SCHEMA, "size": 17, "media_type": "application/schema+json"},
            "output": {"digest": SCHEMA, "size": 17, "media_type": "application/schema+json"},
            "effects": {"reads": ["workspace"], "writes": [], "network": [], "process": false, "delegates": false, "spend": false},
            "minimum": {},
            "support": {"bounds": {"wall_ms": "unknown"}, "cancellation": "unsupported", "idempotency": "none", "evidence": []},
            "binding_contract": {"operation": "mine", "interface": "host.v1"}
        })
    }

    #[test]
    fn a_public_capability_definition_is_parsed_and_a_blob_is_not() {
        let good = signer().sign(
            30,
            cap::DISCOVERY_KIND,
            vec![
                Tag::new(vec!["d".into(), "demo".into()]),
                Tag::new(vec!["t".into(), cap::CAP_MARKER.into()]),
            ],
            definition().to_string(),
        );
        assert!(admit(&good).is_ok());
        let mut bad = good.clone();
        bad.content = "{}".into();
        bad.kind = cap::DISCOVERY_KIND;
        assert!(admit(&bad).is_err());
        assert!(!relay_ok_is_execution());
        assert!(!eose_is_complete());
        assert_eq!(
            EventClass::from_kind(ext::RELEASE_KIND),
            EventClass::Regular
        );
        assert_eq!(
            EventClass::from_kind(cap::DISCOVERY_KIND),
            EventClass::Addressable
        );
        assert_eq!(EventClass::from_kind(25_920), EventClass::Ephemeral);
    }

    #[test]
    fn private_policy_and_run_ciphertext_are_not_read_as_programs() {
        let author = signer();
        let policy = author.sign(
            31,
            PREFERENCE_KIND,
            vec![
                Tag::new(vec!["t".into(), cap::PRIVATE_POLICY_MARKER.into()]),
                Tag::new(vec!["p".into(), author.pubkey().into()]),
                Tag::new(vec!["d".into(), "ab".repeat(32)]),
            ],
            "bm90LWEgcHJvZ3JhbQ==".into(),
        );
        assert!(admit(&policy).is_ok());
        assert!(requires_author_auth(&policy));
        let mut plaintext = policy.clone();
        plaintext.content = definition().to_string();
        assert!(admit(&plaintext).is_err());

        let record = author.sign(
            32,
            RECORD_KIND,
            vec![
                Tag::new(vec!["p".into(), author.pubkey().into()]),
                Tag::new(vec!["h".into(), "cd".repeat(32)]),
                Tag::new(vec!["t".into(), run::MARKER.into()]),
            ],
            "bm90LWEganJvdXJuYWw=".into(),
        );
        assert!(admit(&record).is_ok());
        let mut json_record = record.clone();
        json_record.content = r#"{"steps":[]}"#.into();
        assert!(admit(&json_record).is_err());
        assert!(requires_author_auth(&record));
    }

    fn market_offering_body() -> Value {
        let author = signer();
        json!({
            "v": market_contracts::OFFERING_SCHEMA, "requires": [],
            "provider": author.pubkey(), "offer": "example",
            "capability": {
                "id": format!("{}:example/work", author.pubkey()),
                "artifact": {"digest": SCHEMA, "size": 17,
                    "media_type": "application/json", "schema": "openagents.cap.v1"}
            },
            "profiles": ["openagents.labor.v1"],
            "payment_profiles": [market_contracts::FREE_PROFILE], "networks": [],
            "summary": "A discovery fixture", "price_hint_msat": 0,
            "capacity_hint": null, "valid_until": 100
        })
    }

    fn market_offering(body: &Value) -> Event {
        let bytes = crate::contracts::jcs(body).unwrap();
        let digest = crate::contracts::digest_bytes(&bytes);
        signer().sign(
            30,
            market_contracts::OFFERING_KIND,
            vec![
                Tag::new(vec!["t".into(), "oa:market-offering:v1".into()]),
                Tag::new(vec!["x".into(), digest[7..].into()]),
            ],
            String::from_utf8(bytes).unwrap(),
        )
    }

    fn market_head(body: &Value) -> Event {
        signer().sign(
            31,
            market_contracts::HEAD_KIND,
            vec![
                Tag::new(vec!["d".into(), "example".into()]),
                Tag::new(vec!["t".into(), "oa:market-head:v1".into()]),
            ],
            String::from_utf8(crate::contracts::jcs(body).unwrap()).unwrap(),
        )
    }

    #[test]
    fn market_discovery_admission_checks_signed_closed_content() {
        let body = market_offering_body();
        let good = market_offering(&body);
        assert!(admit(&good).is_ok());
        assert!(!requires_author_auth(&good));
        for (field, value) in [
            ("requires", json!(["future"])),
            ("grant", json!("run-now")),
            ("provider", json!(PUB)),
            ("payment_profiles", json!(["unknown"])),
            ("valid_until", json!(30)),
        ] {
            let mut bad = body.clone();
            bad[field] = value;
            assert!(admit(&market_offering(&bad)).is_err(), "accepted {field}");
        }
        let mut forged = good;
        forged.content.push(' ');
        assert!(admit(&forged).is_err());
    }

    #[test]
    fn market_head_admission_does_not_resolve_its_offering_reference() {
        let author = signer();
        let body = json!({
            "v": market_contracts::HEAD_SCHEMA, "requires": [],
            "provider": author.pubkey(), "offer": "example",
            "offering": {"id": "bb".repeat(32), "pubkey": author.pubkey(),
                "kind": market_contracts::OFFERING_KIND},
            "status": "active", "valid_until": 100
        });
        let head = market_head(&body);
        // Admission validates the signed claim without inventing referenced bytes.
        assert!(admit(&head).is_ok());
        let other =
            market_contracts::parse_offering(&market_offering(&market_offering_body())).unwrap();
        assert!(market_contracts::check_head(&head, &other, 40).is_err());
        for (field, value) in [
            ("status", json!("confirmed")),
            ("offer", json!("different")),
            ("execute", json!(true)),
            ("valid_until", json!(31)),
        ] {
            let mut bad = body.clone();
            bad[field] = value;
            assert!(admit(&market_head(&bad)).is_err(), "accepted {field}");
        }
        let mut bad = body;
        bad["offering"]["kind"] = json!(market_contracts::HEAD_KIND);
        assert!(admit(&market_head(&bad)).is_err());
        assert!(!relay_ok_is_execution());
    }
}
