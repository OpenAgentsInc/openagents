//! Real, public Confidential Space evidence from the #11787 test image.
use serde_json::Value;
use sha2::{Digest, Sha256};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/layertree-tdx-probe.json")).unwrap()
}

#[test]
fn the_signet_probe_token_binds_its_image_and_test_key_hash() {
    let probe = fixture();
    let claims = oa_att::token::verify(
        probe["token"].as_str().unwrap(),
        "openagents.layertree.route.v1",
        1_792_135_600,
    )
    .unwrap();
    assert_eq!(claims.hwmodel, "GCP_INTEL_TDX");
    assert_eq!(
        claims.image_digest,
        "sha256:92c8b535757b1fa43f2ab85882d0fe58c42dbe29562c3cfcbb5b6953130448c6"
    );
    assert_eq!(claims.nonces, [probe["nonce"].as_str().unwrap()]);
    assert_eq!(claims.dbgstat, "disabled-since-boot");
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
    assert!(!probe["transaction_signing"].as_bool().unwrap());
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
