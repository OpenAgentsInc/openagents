//! The NIP-HOST CAP operation set: the checked-in manifest is the one a host
//! builds, it passes the NIP-CAP validator, its schema references pin the
//! checked-in schemas, and a published manifest reads back only from the
//! pinned host. Runs without the host feature.

use coder_access::cj::{ANSWER_SCHEMA, CALL_SCHEMA, Capability, OPERATIONS, SLUG, definition};
use nostr::cap::{self, Profile};
use nostr::contracts::{digest_bytes, prepare_closure};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const RELAY: &str = "wss://relay.openagents.com";

/// The fixture's host: the x-only key of secret key 1.
fn fixture_host() -> (SecretKey, String) {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    let secret = SecretKey::from_byte_array(bytes).unwrap();
    let host = coder_access::protocol::pubkey(&secret);
    (secret, host)
}

fn checked_in() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/host-access-capability.json"
    ))
    .expect("capability fixture");
    serde_json::from_str(&text).expect("capability JSON")
}

#[test]
fn checked_in_manifest_is_the_one_a_host_builds_and_validates() {
    let (_, host) = fixture_host();
    let fixture = checked_in();
    let built = definition(&host, &[RELAY.to_owned()]);
    assert_eq!(fixture["definition"], built, "regenerate the fixture");

    let parsed = cap::parse_definition(&fixture["definition"]).expect("NIP-CAP validator");
    assert_eq!(parsed.profile, Profile::Adapter);
    assert_eq!(parsed.transport, "nostr-cj");
    assert_eq!(parsed.component, SLUG);
    assert_eq!(parsed.idempotency, "request_attempt");
    assert_eq!(
        fixture["definition"]["binding_contract"]["operations"],
        json!(OPERATIONS)
    );

    // The schema references pin the checked-in schema documents, and the
    // shared evaluator accepts both.
    for (field, bytes) in [("input", CALL_SCHEMA), ("output", ANSWER_SCHEMA)] {
        let reference = &fixture["definition"][field];
        assert_eq!(reference["digest"], digest_bytes(bytes));
        assert_eq!(reference["size"], bytes.len());
        assert_eq!(reference["media_type"], "application/schema+json");
    }
    let documents = BTreeMap::from([
        (digest_bytes(CALL_SCHEMA), CALL_SCHEMA.to_vec()),
        (digest_bytes(ANSWER_SCHEMA), ANSWER_SCHEMA.to_vec()),
    ]);
    prepare_closure(&documents).expect("the schemas use supported keywords");
}

#[test]
fn a_published_manifest_reads_back_only_from_its_host() {
    let (secret, host) = fixture_host();
    let capability = Capability::new(&host, vec![RELAY.to_owned()]).unwrap();
    let event = capability.manifest(&secret, 1_800_000_000).unwrap();
    assert_eq!(event.kind, cap::DISCOVERY_KIND);
    let definition = cap::parse_definition(&capability.definition().clone()).unwrap();
    cap::check_discovery_tags(&event.tags, &definition).unwrap();
    assert_eq!(Capability::from_event(&event, &host).unwrap(), capability);

    // Another signer cannot publish this host's capability, and a pin on
    // another host refuses this one.
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    assert!(capability.manifest(&other, 1_800_000_000).is_err());
    let stranger = coder_access::protocol::pubkey(&other);
    assert!(Capability::from_event(&event, &stranger).is_err());

    // A manifest whose definition differs from the one this binding builds
    // is refused, even when validly signed by the host.
    let mut altered = capability.definition().clone();
    altered["effects"]["spend"] = json!(true);
    let signer =
        nostr::domain::RelaySigner::from_secret_hex(&secret.display_secret().to_string()).unwrap();
    let forged = signer.sign(
        1_800_000_000,
        cap::DISCOVERY_KIND,
        event.tags.clone(),
        json!({"definition": altered}).to_string(),
    );
    assert!(Capability::from_event(&forged, &host).is_err());
}
