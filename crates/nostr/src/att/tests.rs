#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

const NOW: u64 = 1_791_400_000;

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
}

fn release(publisher: &RelaySigner) -> Release {
    Release {
        v: RELEASE_V.into(),
        requires: Vec::new(),
        workload: "clef-decisions".into(),
        publisher: publisher.pubkey().into(),
        image: Image {
            reference: "us-central1-docker.pkg.dev/openagentsgemini/openagents/att-provider".into(),
            digest: format!("sha256:{}", "ab".repeat(32)),
        },
        platforms: vec![Platform {
            kind: "gcp-confidential-space".into(),
            hwmodel: "GCP_INTEL_TDX".into(),
            support: "STABLE".into(),
        }],
        measurements: Vec::new(),
        gpu: None,
        models: vec![Model {
            id: "clef-flash".into(),
            digest: format!("sha256:{}", "cd".repeat(32)),
        }],
        components: vec![Component {
            name: "psionic-openai-server".into(),
            digest: format!("sha256:{}", "ef".repeat(32)),
        }],
        source: Source {
            repo: "https://github.com/OpenAgentsInc/openagents".into(),
            commit: "1".repeat(40),
            recipe: "deploy/att/cloudbuild.yaml".into(),
            recipe_digest: format!("sha256:{}", "12".repeat(32)),
        },
        rebuilds: Vec::new(),
        transparency: Vec::new(),
        changes: "First release.".into(),
        published_at: NOW - 1_000,
    }
}

fn head(release_id: &str, effective_at: u64) -> Head {
    Head {
        v: HEAD_V.into(),
        requires: Vec::new(),
        workload: "clef-decisions".into(),
        generation: 3,
        notice_seconds: 600,
        admitted: vec![Admitted {
            release: release_id.into(),
            effective_at,
            retire_at: None,
        }],
        emergency: None,
    }
}

fn endpoint(key: &RelaySigner, release_id: &str) -> Endpoint {
    Endpoint {
        v: ENDPOINT_V.into(),
        requires: Vec::new(),
        release: release_id.into(),
        endpoint: key.pubkey().into(),
        hpke: None,
        binding: binding(key.pubkey(), None, release_id).unwrap(),
        evidence: vec![Evidence {
            kind: "gcp-confidential-space-token".into(),
            format: "pki".into(),
            token: "eyJ.x.y".into(),
        }],
        operator: signer(9).pubkey().into(),
        issued_at: NOW,
        valid_until: NOW + 3_600,
    }
}

#[test]
fn binding_is_the_documented_hash() {
    let key = "11".repeat(32);
    let release = "22".repeat(32);
    let mut bytes = b"openagents.att.v1\0".to_vec();
    bytes.extend([0x11; 32]);
    bytes.extend([0x22; 32]);
    assert_eq!(binding(&key, None, &release).unwrap(), sha256_hex(&bytes));
    let hpke = [0x33; 32];
    let mut with = b"openagents.att.v1\0".to_vec();
    with.extend([0x11; 32]);
    with.extend([0x33; 32]);
    with.extend([0x22; 32]);
    assert_eq!(
        binding(&key, Some(&hpke), &release).unwrap(),
        sha256_hex(&with)
    );
    assert!(binding("zz", None, &release).is_err());
}

#[test]
fn a_release_round_trips_and_names_its_publisher() {
    let publisher = signer(1);
    let body = release(&publisher);
    let event = release_event(&publisher, &body, NOW - 900).unwrap();
    assert_eq!(event.kind, 3_202);
    assert_eq!(parse_release(&event).unwrap(), body);
    // Another key cannot sign it.
    assert!(release_event(&signer(2), &body, NOW).is_err());
    // A changed content breaks the x tag and the signature.
    let mut forged = event.clone();
    forged.content = forged.content.replace("ab", "ac");
    assert!(parse_release(&forged).is_err());
}

#[test]
fn a_head_admits_after_the_notice_delay_only() {
    let publisher = signer(1);
    let body = release(&publisher);
    let event = release_event(&publisher, &body, NOW - 900).unwrap();
    let ok = head(&event.id, body.published_at + 600);
    let signed = head_event(&publisher, &ok, NOW).unwrap();
    let parsed = parse_head(&signed).unwrap();
    assert!(parsed.admits(&event.id, &body, NOW).is_ok());
    // Inside the notice delay.
    let early = head(&event.id, body.published_at + 599);
    assert!(
        early
            .admits(&event.id, &body, NOW)
            .unwrap_err()
            .contains("notice")
    );
    // Not yet effective.
    let later = head(&event.id, NOW + 10);
    assert!(later.admits(&event.id, &body, NOW).is_err());
    // Not listed.
    assert!(ok.admits(&"00".repeat(32), &body, NOW).is_err());
    // Retired.
    let mut retired = ok.clone();
    retired.admitted[0].retire_at = Some(NOW - 1);
    assert!(retired.admits(&event.id, &body, NOW).is_err());
}

#[test]
fn an_emergency_release_waits_at_least_a_day() {
    let publisher = signer(1);
    let mut body = release(&publisher);
    body.published_at = NOW - 90_000;
    let event = release_event(&publisher, &body, NOW).unwrap();
    let mut slow = head(&event.id, body.published_at + 86_400);
    slow.notice_seconds = 604_800;
    assert!(slow.admits(&event.id, &body, NOW).is_err());
    slow.emergency = Some(Emergency {
        release: event.id.clone(),
        reason: "a key leak".into(),
    });
    assert!(slow.admits(&event.id, &body, NOW).is_ok());
    slow.admitted[0].effective_at = body.published_at + 86_399;
    assert!(slow.admits(&event.id, &body, NOW).is_err());
}

#[test]
fn a_head_never_rolls_back() {
    let h = head(&"aa".repeat(32), NOW);
    assert_eq!(check_generation(None, &h).unwrap(), 3);
    assert_eq!(check_generation(Some(2), &h).unwrap(), 3);
    assert_eq!(check_generation(Some(3), &h).unwrap(), 3);
    assert!(
        check_generation(Some(4), &h)
            .unwrap_err()
            .contains("rollback")
    );
}

#[test]
fn an_endpoint_is_signed_by_its_key_and_bound_to_its_release() {
    let key = signer(5);
    let release_id = "ab".repeat(32);
    let body = endpoint(&key, &release_id);
    let instance = "77".repeat(32);
    let address = format!("30202:{}:clef-decisions", signer(1).pubkey());
    let event = endpoint_event(&key, &body, &instance, &address).unwrap();
    let record = parse_endpoint(&event).unwrap();
    assert_eq!(record.body, body);
    assert_eq!(record.head, address);
    assert_eq!(
        record.address(),
        format!("30203:{}:{instance}", key.pubkey())
    );
    assert!(body.current(NOW + 10).is_ok());
    assert!(body.current(NOW + 3_600).is_err());

    // An unbound key: the binding names another key.
    let mut unbound = body.clone();
    unbound.binding = binding(signer(6).pubkey(), None, &release_id).unwrap();
    assert!(unbound.validate().unwrap_err().contains("binding"));
    // Another key cannot sign the endpoint.
    assert!(endpoint_event(&signer(6), &body, &instance, &address).is_err());
    // Too long a validity.
    let mut long = body.clone();
    long.valid_until = long.issued_at + 3_601;
    assert!(long.validate().is_err());
    // A binding for another release.
    let mut moved = body;
    moved.release = "cd".repeat(32);
    assert!(moved.validate().is_err());
}

#[test]
fn sealed_fields_round_trip_through_the_worker_check() {
    let (requires, attested) = sealed_fields("30203:aa:bb", &"cd".repeat(32), Level::TeeCloud);
    let payload = json!({"requires": requires, "attested": attested});
    assert_eq!(
        check_sealed(&payload, "30203:aa:bb", &"cd".repeat(32)).unwrap(),
        Level::TeeCloud
    );
    assert!(check_sealed(&payload, "30203:aa:cc", &"cd".repeat(32)).is_err());
    assert!(check_sealed(&payload, "30203:aa:bb", &"ce".repeat(32)).is_err());
    assert!(check_sealed(&json!({}), "30203:aa:bb", &"cd".repeat(32)).is_err());
}

#[test]
fn hpke_keys_are_base64() {
    let key = signer(5);
    let release_id = "ab".repeat(32);
    let mut body = endpoint(&key, &release_id);
    let raw = [7u8; 32];
    body.hpke = Some("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=".into());
    body.binding = binding(key.pubkey(), Some(&raw), &release_id).unwrap();
    body.validate().unwrap();
    body.hpke = Some("not base64!".into());
    assert!(body.validate().is_err());
}
