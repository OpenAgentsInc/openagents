//! Verify the retained synthetic vault composition from a real TDX workload.
//! The client shared its process and used LocalKms; this is not production KMS
//! or remote-client qualification. No private key left the deleted machine.
use nostr::{att::Level, contracts::digest_bytes, domain::Event};
use serde_json::Value;
use vault_sealed::{
    Signed,
    client::{Client, VerifiedEndpoint, verify_endpoint},
};

const PUBLISHER: &str = "691f3a0bfb30699f9fdc278517d6952ab52564bc49253f6e0f2631128a726405";
const IMAGE: &str = "sha256:17ed47bc26e6f48edef97ea90b184223135dba885400d4451098d6ad5a20df2e";
const SOURCE: &str = "b73efa585dcdafc9be0405981bcc9f1160b92e62";

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../vault-sealed/tests/fixtures/confidential-space-vault-tdx-2026-10-16.json"
    ))
    .unwrap()
}

fn verify(raw: &Value) -> Result<VerifiedEndpoint, String> {
    let event = |name: &str| -> Result<Event, String> {
        serde_json::from_value(raw[name].clone()).map_err(|e| e.to_string())
    };
    let completion: Signed =
        serde_json::from_value(raw["completion"].clone()).map_err(|e| e.to_string())?;
    let now = completion.body["captured_at"]
        .as_u64()
        .ok_or("missing time")?;
    if raw["schema"] != "openagents.vault-cloud-fixture.v1" || raw["publisher"] != PUBLISHER {
        return Err("unexpected research capture".into());
    }
    let records = oa_att::Records {
        release: event("release")?,
        head: event("head")?,
        endpoint: event("endpoint")?,
        beacon: None,
    };
    let endpoint = verify_endpoint(&records, PUBLISHER, now)?;
    completion.verify(&endpoint.endpoint)?;
    let body = &completion.body;
    let release = nostr::att::parse_release(&records.release)?;
    if body["v"] != "openagents.vault-cloud-composition.v1"
        || body["release"] != records.release.id
        || body["endpoint"] != records.endpoint.id
        || body["image_digest"] != IMAGE
        || release.image.digest != IMAGE
        || body["source_commit"] != SOURCE
        || release.source.commit != SOURCE
        || body["level"] != "tee-cloud"
        || endpoint.level != Level::TeeCloud
        || body["answer_bytes"] != serde_json::json!([20])
        || body["client_location"] != "same-workload"
        || body["kms"] != "ephemeral-local-fixture"
        || body["production_qualification"] != false
        || body["independent_rebuild"] != false
    {
        return Err("the completion does not describe the qualified research run".into());
    }
    for check in [
        "one_use_refused",
        "revoked_refused",
        "unbound_hpke_refused",
        "expired_refused",
    ] {
        if body[check] != true {
            return Err("missing negative check".into());
        }
    }
    for name in ["object", "request", "reply"] {
        let bytes = oa_vault::unb64(raw[name].as_str().ok_or("missing ciphertext")?)
            .map_err(|e| e.to_string())?;
        if body[format!("{name}_sha256")] != digest_bytes(&bytes) {
            return Err("the ciphertext differs from the signed completion".into());
        }
    }
    let certificate: Signed =
        serde_json::from_value(raw["certificate"].clone()).map_err(|e| e.to_string())?;
    // A new client can verify the real endpoint's public system certificate.
    // It cannot open the old object: that request's share and keys were not exported.
    let mut client = Client::new(
        oa_vault::Vmk::from_bytes([0x17; 32]),
        &"18".repeat(32),
        secp256k1::SecretKey::from_byte_array([0x19; 32]).unwrap(),
    );
    client.accept_system_key(&endpoint, &certificate, now)?;
    Ok(endpoint)
}

#[test]
fn a_captured_attested_vault_run_verifies_its_endpoint_and_completion() {
    let raw = fixture();
    let endpoint = verify(&raw).unwrap();
    assert_eq!(
        endpoint.event,
        vault_sealed::id_bytes(raw["endpoint"]["id"].as_str().unwrap()).unwrap()
    );
    assert!(endpoint.valid_until > raw["completion"]["body"]["captured_at"].as_u64().unwrap());
    assert_eq!(
        raw["audit"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["result"] == "granted")
            .count(),
        1
    );
}

#[test]
fn changed_vault_records_certificates_ciphertexts_and_completion_are_refused() {
    let original = fixture();
    for field in ["object", "request", "reply"] {
        let mut changed = original.clone();
        changed[field] = Value::String(oa_vault::b64(b"different ciphertext"));
        assert!(verify(&changed).is_err(), "accepted changed {field}");
    }
    let mut changed = original.clone();
    changed["completion"]["body"]["answer_bytes"] = serde_json::json!([0]);
    assert!(verify(&changed).is_err());
    let mut changed = original.clone();
    changed["certificate"]["body"]["epoch"] = serde_json::json!(999);
    assert!(verify(&changed).is_err());
    let mut changed = original.clone();
    let mut endpoint: Value =
        serde_json::from_str(changed["endpoint"]["content"].as_str().unwrap()).unwrap();
    endpoint["hpke"] = Value::String(oa_vault::b64(&[0x71; 32]));
    changed["endpoint"]["content"] = Value::String(serde_json::to_string(&endpoint).unwrap());
    assert!(verify(&changed).is_err());
    let endpoint = verify(&original).unwrap();
    let event = |name: &str| -> Event { serde_json::from_value(original[name].clone()).unwrap() };
    let records = oa_att::Records {
        release: event("release"),
        head: event("head"),
        endpoint: event("endpoint"),
        beacon: None,
    };
    assert!(verify_endpoint(&records, PUBLISHER, endpoint.valid_until).is_err());
}
