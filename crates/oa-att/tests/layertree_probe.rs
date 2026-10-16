//! Real, public Confidential Space evidence from the #11787 test image.
use serde_json::Value;
use sha2::{Digest, Sha256};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/layertree-tdx-probe.json")).unwrap()
}

fn verify_probe(probe: Value, digest: &str, now: u64) -> oa_att::token::Claims {
    let claims = oa_att::token::verify(
        probe["token"].as_str().unwrap(),
        "openagents.layertree.route.v1",
        now,
    )
    .unwrap();
    assert_eq!(claims.hwmodel, "GCP_INTEL_TDX");
    assert_eq!(claims.image_digest, digest);
    assert_eq!(claims.nonces, [probe["nonce"].as_str().unwrap()]);
    assert_eq!(claims.dbgstat, "disabled-since-boot");
    assert!(claims.support.iter().any(|value| value == "STABLE"));
    assert_eq!(
        &probe["report_data"].as_str().unwrap()[..64],
        probe["nonce"].as_str().unwrap()
    );
    let tag = Sha256::digest(b"LayerTreeTEE/report-data");
    let mut hash = Sha256::new();
    hash.update(tag);
    hash.update(tag);
    for field in ["enclave_key", "operator_key", "tree_id"] {
        let text = probe[field].as_str().unwrap();
        let bytes: Vec<_> = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes.len(), 32);
        hash.update(bytes);
    }
    hash.update((probe["depth"].as_u64().unwrap() as u32).to_le_bytes());
    hash.update(probe["dust_min"].as_u64().unwrap().to_le_bytes());
    hash.update((probe["unroll_delay"].as_u64().unwrap() as u16).to_le_bytes());
    assert!(probe["migration_m"].is_null());
    hash.update([0]);
    let expected: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(claims.nonces, [expected]);
    assert_eq!(
        &probe["report_data"].as_str().unwrap()[64..],
        "0".repeat(64)
    );
    if let Some(signing) = probe["transaction_signing"].as_bool() {
        assert!(!signing);
    } else {
        assert!(!probe["accepts_external_transactions"].as_bool().unwrap());
        assert_eq!(probe["ephemeral_keys_after_signing"], 0);
    }
    claims
}

#[test]
fn another_audience_and_a_modified_token_are_refused() {
    let probe = fixture();
    let token = probe["token"].as_str().unwrap();
    assert!(oa_att::token::verify(token, "another-audience", 1_792_135_600).is_err());
    let mut parts: Vec<String> = token.split('.').map(str::to_owned).collect();
    let replacement = if parts[1].starts_with('a') { "b" } else { "a" };
    parts[1].replace_range(0..1, replacement);
    assert!(
        oa_att::token::verify(
            &parts.join("."),
            "openagents.layertree.route.v1",
            1_792_135_600
        )
        .is_err()
    );
    assert!(oa_att::token::verify(token, "openagents.layertree.route.v1", u64::MAX / 2).is_err());
}

#[test]
fn the_signet_probe_token_binds_its_image_and_test_key_hash() {
    verify_probe(
        fixture(),
        "sha256:92c8b535757b1fa43f2ab85882d0fe58c42dbe29562c3cfcbb5b6953130448c6",
        1_792_135_600,
    );
}

#[test]
fn the_reproduced_image_reports_its_logged_digest() {
    verify_probe(
        serde_json::from_str(include_str!("fixtures/layertree-reproduced-tdx-probe.json")).unwrap(),
        "sha256:97ebee658897e3e655c18e01c043b6ab9394ff63ab06b056aea6a6db0127671f",
        1_792_136_800,
    );
}

#[test]
fn a_guest_generated_key_is_bound_to_the_sealing_image() {
    let probe: Value =
        serde_json::from_str(include_str!("fixtures/layertree-key-first-tdx.json")).unwrap();
    assert_eq!(probe["restored"], false);
    assert_eq!(
        probe["enclave_key"],
        "a90d75983b148a0d5f684e6bcc018bd55a95237854ca0c8ab77ec4df57438c0d"
    );
    verify_probe(
        probe,
        "sha256:79bf22d374f8c33a58f4c126ce36da9f729501d881ac80b48707d007653bf100",
        1_792_138_800,
    );
}

#[test]
fn another_boot_recovers_the_same_attested_key() {
    let first: Value =
        serde_json::from_str(include_str!("fixtures/layertree-key-first-tdx.json")).unwrap();
    let restored: Value =
        serde_json::from_str(include_str!("fixtures/layertree-key-restored-tdx.json")).unwrap();
    assert_eq!(restored["restored"], true);
    assert_eq!(first["enclave_key"], restored["enclave_key"]);
    assert_eq!(first["report_data"], restored["report_data"]);
    assert_ne!(first["token"], restored["token"]);
    let claims = verify_probe(
        restored,
        "sha256:79bf22d374f8c33a58f4c126ce36da9f729501d881ac80b48707d007653bf100",
        1_792_138_800,
    );
    assert!(claims.iat > 1_792_138_342);
}
