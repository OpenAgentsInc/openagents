//! The client checks against real records: a release, head and endpoint
//! captured from wss://relay.openagents.com on 2026-10-10, whose endpoint
//! carries a Google Confidential Space token from the Intel TDX machine
//! `oa-att-tdx-1` (#11241). Each negative case changes one input.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use oa_att::nostr::att::Level;
use oa_att::nostr::domain::Event;
use oa_att::{Policy, Records, Tamper};
use serde_json::Value;

fn fixture() -> (Records, Policy, u64) {
    let raw: Value = serde_json::from_str(include_str!(
        "fixtures/confidential-space-tdx-2026-10-10.json"
    ))
    .unwrap();
    let event = |key: &str| -> Event { serde_json::from_value(raw[key].clone()).unwrap() };
    let records = Records {
        release: event("release"),
        head: event("head"),
        endpoint: event("endpoint"),
        beacon: None,
    };
    let policy = Policy {
        publisher: raw["publisher"].as_str().unwrap().into(),
        workload: oa_att::WORKLOAD.into(),
        required: Level::TeeCloud,
        seen_generation: None,
    };
    (records, policy, raw["captured_at"].as_u64().unwrap())
}

#[test]
fn the_real_endpoint_verifies_end_to_end() {
    let (records, policy, now) = fixture();
    let parsed = oa_att::parse(&records, &policy, now).unwrap();
    let claims = oa_att::chain(&parsed, now).unwrap();
    assert_eq!(claims.hwmodel, "GCP_INTEL_TDX");
    assert_eq!(claims.dbgstat, "disabled-since-boot");
    assert_eq!(claims.chain.len(), 3);
    let measured = oa_att::measure(&parsed, &claims, now, Tamper::None).unwrap();
    assert_eq!(measured.reported, parsed.release.image.digest);
    let bound = oa_att::bind(&parsed, &claims, &policy, None).unwrap();
    assert_eq!(bound.level, Level::TeeCloud);
}

#[test]
fn a_changed_measurement_is_refused() {
    let (records, policy, now) = fixture();
    let parsed = oa_att::parse(&records, &policy, now).unwrap();
    let claims = oa_att::chain(&parsed, now).unwrap();
    let why = oa_att::measure(&parsed, &claims, now, Tamper::Measurement).unwrap_err();
    assert!(why.0.contains("fingerprint"), "{why}");
}

#[test]
fn an_unbound_key_is_refused() {
    let (records, policy, now) = fixture();
    let parsed = oa_att::parse(&records, &policy, now).unwrap();
    let claims = oa_att::chain(&parsed, now).unwrap();
    let other = "11".repeat(32);
    let why = oa_att::bind(&parsed, &claims, &policy, Some(&other)).unwrap_err();
    assert!(why.0.contains("not bound"), "{why}");
}

#[test]
fn expired_evidence_is_refused() {
    let (records, policy, now) = fixture();
    assert!(oa_att::parse(&records, &policy, now + 4_000).is_err());
    let parsed = oa_att::parse(&records, &policy, now).unwrap();
    assert!(oa_att::chain(&parsed, now + 4_000).is_err());
}

#[test]
fn a_head_rollback_is_refused() {
    let (records, mut policy, now) = fixture();
    policy.seen_generation = Some(u64::MAX);
    assert!(
        oa_att::parse(&records, &policy, now)
            .unwrap_err()
            .0
            .contains("rollback")
    );
}

#[test]
fn another_publisher_is_refused() {
    let (records, mut policy, now) = fixture();
    policy.publisher = "22".repeat(32);
    assert!(oa_att::parse(&records, &policy, now).is_err());
}

#[test]
fn a_changed_claim_breaks_google_signature() {
    let (records, policy, now) = fixture();
    let parsed = oa_att::parse(&records, &policy, now).unwrap();
    let token = &parsed.endpoint.body.evidence[0].token;
    let parts: Vec<&str> = token.split('.').collect();
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let claims = String::from_utf8(engine.decode(parts[1]).unwrap()).unwrap();
    // A debug machine would say so here; Google's signature then fails.
    let forged = claims.replace("disabled-since-boot", "enabled");
    assert_ne!(forged, claims);
    let token = format!("{}.{}.{}", parts[0], engine.encode(forged), parts[2]);
    let why = oa_att::token::verify(&token, oa_att::nostr::att::TOKEN_AUDIENCE, now).unwrap_err();
    assert!(why.0.contains("signature"), "{why}");
}
