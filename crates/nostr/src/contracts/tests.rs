use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;
use crate::domain::{RelaySigner, Tag};
use crate::nip44::{self, conversation_key};

const PUB: &str = "abababababababababababababababababababababababababababababababab";
const ID: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

fn artifact(bytes: &[u8]) -> Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": "text/plain",
        "schema": "openagents.fixture.v1"
    })
}

fn definition(bytes: &[u8]) -> Value {
    json!({
        "id": format!("{PUB}:arena/quest"),
        "artifact": artifact(bytes)
    })
}

#[test]
fn unknown_fields_and_features_refuse_while_meta_stays_inert() {
    let mut lock = sample_lock();
    lock["note"] = json!("hidden instruction");
    assert_eq!(
        parse_lock(&lock).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    lock.as_object_mut().unwrap().remove("note");
    lock["requires"] = json!(["future-effect"]);
    assert_eq!(
        parse_lock(&lock).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    lock["requires"] = json!([]);
    lock["meta"] = json!({"display": "ignored"});
    assert!(parse_lock(&lock).is_ok());
}

#[test]
fn lock_keeps_pinned_bytes_and_refuses_cycles_and_missing_content() {
    let bytes = b"quest";
    let lock = parse_lock(&sample_lock()).expect("lock");
    let mut store = BTreeMap::new();
    store.insert(digest_bytes(bytes), bytes.to_vec());
    store.insert(digest_bytes(b"latest"), b"latest".to_vec());
    let resolved = resolve_lock(&lock, &store, ResolveLimits::default()).expect("resolved");
    assert_eq!(
        resolved
            .by_digest
            .get(&digest_bytes(bytes))
            .map(Vec::as_slice),
        Some(bytes.as_slice())
    );
    assert!(!resolved.by_digest.contains_key(&digest_bytes(b"latest")));

    store.remove(&digest_bytes(bytes));
    assert_eq!(
        resolve_lock(&lock, &store, ResolveLimits::default())
            .unwrap_err()
            .code,
        RefusalCode::ContentUnavailable
    );

    let mut cyclic = sample_lock();
    cyclic["entries"][0]["dependencies"] = json!([format!("{PUB}:arena/quest")]);
    assert_eq!(
        parse_lock(&cyclic).unwrap_err().code,
        RefusalCode::Incompatible
    );
}

#[test]
fn effects_reject_paths_and_children_cannot_spend_an_unknown_hold() {
    let effects = parse_effects(&json!({
        "reads": ["deposit"],
        "writes": [],
        "network": [],
        "process": false,
        "delegates": false,
        "spend": false
    }))
    .expect("effects");
    assert!(effects.reads == ["deposit"] && !effects.spend);
    assert_eq!(
        parse_effects(&json!({
            "reads": ["../secret"],
            "writes": [],
            "network": [],
            "process": false,
            "delegates": false,
            "spend": false
        }))
        .unwrap_err()
        .code,
        RefusalCode::Malformed
    );

    let bound = parse_bound(&json!({
        "bound": "wall_ms",
        "ceiling": 1000,
        "minimum": 2000,
        "assurance": "host",
        "mechanism": "deadline"
    }));
    assert_eq!(bound.unwrap_err().code, RefusalCode::Incompatible);
    assert_eq!(
        check_enforcement(&[Bound::WallMs], &[]).unwrap_err().code,
        RefusalCode::CannotEnforce
    );

    let parent = parse_reservation(&json!({
        "v": "openagents.reservation.v1",
        "requires": [],
        "id": ID,
        "currency": "COMPUTE",
        "ceiling_microunits": 10,
        "spent_microunits": null,
        "unknown": true
    }))
    .expect("reservation");
    let child = parse_reservation(&json!({
        "v": "openagents.reservation.v1",
        "requires": [],
        "id": PUB,
        "currency": "COMPUTE",
        "ceiling_microunits": 1,
        "spent_microunits": 0,
        "unknown": false
    }))
    .expect("child");
    assert_eq!(
        admit_child(&parent, &child).unwrap_err().code,
        RefusalCode::CannotEnforce
    );
}

#[test]
fn evidence_scope_and_incomplete_derivation() {
    let bytes = b"seen";
    let mut record = json!({
        "v": "openagents.evidence.v1",
        "requires": [],
        "content": artifact(bytes),
        "source": {"kind": "document", "identity": "note", "version": "1"},
        "capture": {"complete": false, "omitted_bytes": 4, "reason": "truncated"},
        "derived_from": [],
        "scope": {"task": ID, "recipients": [], "classification": "public"}
    });
    record = identified(record);
    let original = parse_evidence(&record).expect("evidence");
    assert!(!may_read(&original, PUB));
    assert!(may_read(&original, "local"));

    let mut derived = json!({
        "v": "openagents.evidence.v1",
        "requires": [],
        "content": artifact(b"view"),
        "source": {"kind": "document", "identity": "note", "version": "1"},
        "capture": {"complete": true, "omitted_bytes": 0, "reason": null},
        "derived_from": [original.id],
        "transform": definition(b"view"),
        "parameters": digest_bytes(b"params"),
        "scope": {"task": ID, "recipients": [PUB], "classification": "public"}
    });
    derived = identified(derived);
    let derived = parse_evidence(&derived).expect("derived");
    assert_eq!(
        check_derivation(&[original, derived]).unwrap_err().code,
        RefusalCode::Incompatible
    );
}

#[test]
fn receipt_keeps_outcome_fields_distinct() {
    let bytes = b"patch";
    let mut receipt = json!({
        "v": "openagents.execution-receipt.v1",
        "requires": [],
        "run": ID,
        "step": "verify",
        "attempt": 1,
        "dispatched": true,
        "component": definition(bytes),
        "lock": digest_bytes(b"lock"),
        "input": artifact(bytes),
        "context": digest_bytes(b"context"),
        "authority": artifact(b"authority"),
        "enforcement": artifact(b"plan"),
        "recipient": "local",
        "usage": {"wall_ms": 3, "spend_microunits": null},
        "output": null,
        "outcome": "unknown",
        "verification": "not_run",
        "integration": "not_requested"
    });
    let parsed = parse_receipt(&receipt).expect("receipt");
    assert_eq!(parsed.outcome, Outcome::Unknown);
    assert_eq!(parsed.verification, Verification::NotRun);
    assert_eq!(parsed.integration, Integration::NotRequested);
    assert_eq!(parsed.usage.spend_microunits, None);
    receipt["success"] = json!(true);
    assert_eq!(
        parse_receipt(&receipt).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
}

#[test]
fn envelope_binds_inline_bytes_and_signed_route() {
    let inline = json!({"name": "quest"});
    let canonical = jcs(&inline).expect("jcs");
    let body = json!({
        "v": "openagents.artifact-envelope.v1",
        "requires": [],
        "artifact": {
            "digest": digest_bytes(&canonical),
            "size": canonical.len(),
            "media_type": "application/json",
            "schema": "openagents.fixture.v1"
        },
        "inline": inline,
        "issued_at": 10,
        "retain_until": 20
    });
    let text = String::from_utf8(jcs(&body).expect("body")).expect("utf-8");
    parse_envelope_body(text.as_bytes()).expect("envelope");

    let sender = RelaySigner::from_secret_hex(&"11".repeat(32)).expect("sender");
    let recipient = RelaySigner::from_secret_hex(&"22".repeat(32)).expect("recipient");
    let key = conversation_key(
        &secp256k1::SecretKey::from_byte_array([0x11; 32]).expect("secret"),
        &xonly(&recipient),
    );
    let content = nip44::encrypt(&text, &key, [7; 32]).expect("encrypt");
    let event = sender.sign(
        30,
        ARTIFACT_ENVELOPE_KIND,
        vec![
            Tag::new(vec!["p".into(), recipient.pubkey().into()]),
            Tag::new(vec!["h".into(), ID.into()]),
            Tag::new(vec!["t".into(), ARTIFACT_MARKER.into()]),
        ],
        content,
    );
    let route = envelope_route(&event).expect("route");
    assert_eq!(route.recipient, recipient.pubkey());
    assert_eq!(route.mailbox, ID);
    let opened = nip44::decrypt(&event.content, &key).expect("decrypt");
    parse_envelope_body(opened.as_bytes()).expect("opened");
}

#[test]
fn schema_closure_refuses_network_and_checks_instances() {
    let schema = include_bytes!("../../../../nips/openagents/schemas/artifact-ref.v1.json");
    let digest = digest_bytes(schema);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), schema.to_vec());
    let closure = prepare_closure(&documents).expect("closure");
    let instance = artifact(b"quest");
    validate_instance(&closure, &digest, &instance).expect("instance");
    let mut extra = instance.clone();
    extra["command"] = json!("rm");
    assert_eq!(
        validate_instance(&closure, &digest, &extra)
            .unwrap_err()
            .code,
        RefusalCode::Malformed
    );

    let remote = br#"{"$ref": "https://example.invalid/schema"}"#;
    documents.insert(digest_bytes(remote), remote.to_vec());
    let closure = prepare_closure(&documents).expect("remote schema is only stored");
    assert_eq!(
        validate_instance(&closure, &digest_bytes(remote), &json!({}))
            .unwrap_err()
            .code,
        RefusalCode::UnsupportedFeature
    );
}

fn sample_lock() -> Value {
    let bytes = b"quest";
    json!({
        "v": "openagents.lock.v1",
        "requires": [],
        "root": definition(bytes),
        "entries": [{
            "id": format!("{PUB}:arena/quest"),
            "definition": definition(bytes),
            "dependencies": []
        }]
    })
}

fn identified(mut value: Value) -> Value {
    value.as_object_mut().expect("object").remove("id");
    let digest = digest_value(&value).expect("digest");
    value
        .as_object_mut()
        .expect("object")
        .insert("id".into(), Value::String(digest));
    value
}

fn xonly(signer: &RelaySigner) -> secp256k1::XOnlyPublicKey {
    let mut bytes = [0_u8; 32];
    for (index, pair) in signer.pubkey().as_bytes().chunks(2).enumerate() {
        bytes[index] =
            u8::from_str_radix(std::str::from_utf8(pair).expect("hex"), 16).expect("byte");
    }
    secp256k1::XOnlyPublicKey::from_byte_array(bytes).expect("pubkey")
}
