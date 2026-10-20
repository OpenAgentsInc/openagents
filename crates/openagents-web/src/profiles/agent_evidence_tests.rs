use super::*;
use nostr::domain::{RelaySigner, mint_owner_attestation};

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
}

const NOW: u64 = 1_800_000_000;

fn definition() -> (RelaySigner, Event, agent_public::Definition) {
    let (owner, agent) = (signer(1), signer(2));
    let tag = mint_owner_attestation(
        &secp256k1::SecretKey::from_byte_array([1; 32]).unwrap(),
        agent.pubkey(),
        "created_at<1800003600",
    )
    .unwrap()
    .tag();
    let event = agent_public::definition_event(
        &agent,
        &tag,
        owner.pubkey(),
        &"ab".repeat(32),
        "https://example.org",
        "{}",
        NOW,
    )
    .unwrap();
    let read = agent_public::read_definition(&event).unwrap();
    (owner, event, read)
}

#[test]
fn gateway_receipts_are_owner_selected_checked_and_deduplicated() {
    let (owner, event, definition) = definition();
    let mut receipt =
        receipts::ExecutionReceipt::for_attempt("http", "request-1", 1, "sha256:request");
    receipt.attempt_id = "attempt-1".into();
    receipt.served.model = "model <script>".into();
    receipt.outcome = receipts::Outcome::Answered;
    receipt.timing.latency_ms = Some(87);
    receipt.seal();
    let first = receipts_nostr::event(&owner, &event, &receipt, NOW).unwrap();
    let second = receipts_nostr::event(&owner, &event, &receipt, NOW + 1).unwrap();
    let stranger = signer(3).sign(NOW, first.kind, first.tags.clone(), first.content.clone());
    let mut tampered = first.clone();
    tampered.content.push(' ');
    let future = receipts_nostr::event(&owner, &event, &receipt, NOW + 61).unwrap();
    let receipts = selected_receipts(
        &definition,
        &[first, second.clone(), stranger, tampered, future],
        NOW,
    );
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].0, second.id);
    let markup = render(&Evidence {
        receipts,
        ..Evidence::default()
    })
    .into_string();
    assert!(markup.contains(&format!("/event/{}", second.id)));
    assert!(markup.contains("model &lt;script&gt;"));
    assert!(markup.contains("Answered · 87 ms"));
    crate::copy_guard::assert_plain("agent receipts", &markup);
    let mut retired = definition.clone();
    retired.revoked = true;
    assert!(selected_receipts(&retired, &[second], NOW).is_empty());
    assert!(render(&Evidence::default()).into_string().is_empty());
}

fn release() -> att::Release {
    att::Release {
        v: att::RELEASE_V.into(),
        requires: vec![],
        workload: "test-workload".into(),
        publisher: signer(4).pubkey().into(),
        image: att::Image {
            reference: "https://example.org/image".into(),
            digest: format!("sha256:{}", "ab".repeat(32)),
        },
        platforms: vec![att::Platform {
            kind: "gcp-confidential-space".into(),
            hwmodel: "GCP_INTEL_TDX".into(),
            support: "STABLE".into(),
        }],
        measurements: vec![],
        gpu: None,
        models: vec![],
        components: vec![],
        source: att::Source {
            repo: "https://example.org/repo".into(),
            commit: "a".repeat(40),
            recipe: "build.sh".into(),
            recipe_digest: format!("sha256:{}", "cd".repeat(32)),
        },
        rebuilds: vec![],
        transparency: vec![],
        changes: String::new(),
        published_at: NOW,
    }
}

#[test]
fn only_independent_rebuilds_of_the_exact_source_and_image_count() {
    let (builder, publisher) = (signer(5), signer(4));
    let mut release = release();
    release.rebuilds.push(att::Rebuild {
        builder: builder.pubkey().into(),
        digest: release.image.digest.clone(),
    });
    let mut rebuild = att_release::rebuild::Rebuild {
        v: att_release::rebuild::REBUILD_V.into(),
        requires: vec![],
        workload: release.workload.clone(),
        source: release.source.clone(),
        image_digest: release.image.digest.clone(),
        components: vec![],
        built_at: NOW,
    };
    let good = att_release::rebuild::event(&builder, &rebuild).unwrap();
    let self_signed = att_release::rebuild::event(&publisher, &rebuild).unwrap();
    rebuild.image_digest = format!("sha256:{}", "ef".repeat(32));
    let mismatch = att_release::rebuild::event(&builder, &rebuild).unwrap();
    rebuild.image_digest = release.image.digest.clone();
    rebuild.source.commit = "b".repeat(40);
    let other_source = att_release::rebuild::event(&builder, &rebuild).unwrap();
    assert_eq!(
        matching_rebuilds(
            &release,
            &[
                good.clone(),
                good.clone(),
                self_signed,
                mismatch,
                other_source
            ],
            NOW
        ),
        vec![good.id.clone()]
    );
    let markup = render(&Evidence {
        release: Some(("aa".repeat(32), release.publisher)),
        rebuilds: vec![good.id.clone()],
        ..Evidence::default()
    })
    .into_string();
    assert!(markup.contains(&format!("/event/{}", good.id)));
    assert!(!markup.contains("Verified sealed endpoint"));
    crate::copy_guard::assert_plain("agent attestations", &markup);
}

#[test]
fn a_signed_endpoint_without_hardware_evidence_does_not_verify() {
    let publisher = signer(4);
    let endpoint_key = signer(6);
    let release_body = release();
    let release = att::release_event(&publisher, &release_body, NOW).unwrap();
    let head = att::head_event(
        &publisher,
        &att::Head {
            v: att::HEAD_V.into(),
            requires: vec![],
            workload: release_body.workload.clone(),
            generation: 1,
            notice_seconds: 0,
            admitted: vec![att::Admitted {
                release: release.id.clone(),
                effective_at: NOW,
                retire_at: None,
            }],
            emergency: None,
        },
        NOW,
    )
    .unwrap();
    let endpoint = att::endpoint_event(
        &endpoint_key,
        &att::Endpoint {
            v: att::ENDPOINT_V.into(),
            requires: vec![],
            release: release.id.clone(),
            endpoint: endpoint_key.pubkey().into(),
            hpke: None,
            binding: att::binding(endpoint_key.pubkey(), None, &release.id).unwrap(),
            evidence: vec![],
            operator: publisher.pubkey().into(),
            issued_at: NOW,
            valid_until: NOW + 3600,
        },
        &"ac".repeat(32),
        &format!(
            "{}:{}:{}",
            att::HEAD_KIND,
            publisher.pubkey(),
            release_body.workload
        ),
    )
    .unwrap();
    let records = oa_att::Records {
        release,
        head,
        endpoint,
        beacon: None,
    };
    assert_eq!(
        crate::pages::att::gateway_check(&records, &release_body.workload, NOW)["ok"],
        false
    );
    // The production policy also refuses untrusted release publishers.
    assert!(
        crate::pages::att::gateway_check(&records, &release_body.workload, NOW)["reason"]
            .as_str()
            .unwrap()
            .contains("another publisher")
    );
}

#[test]
fn endpoint_uses_the_att_pages_live_fixture_and_expiry_checks() {
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../oa-att/tests/fixtures/confidential-space-tdx-2026-10-10.json"
    ))
    .unwrap();
    let event = |key: &str| serde_json::from_value::<Event>(raw[key].clone()).unwrap();
    let records = oa_att::Records {
        release: event("release"),
        head: event("head"),
        endpoint: event("endpoint"),
        beacon: None,
    };
    let captured_at = raw["captured_at"].as_u64().unwrap();
    let release = att::parse_release(&records.release).unwrap();
    assert_eq!(
        crate::pages::att::gateway_check(&records, &release.workload, captured_at)["ok"],
        true
    );
    assert_eq!(
        crate::pages::att::gateway_check(&records, &release.workload, captured_at + 7200)["ok"],
        false
    );
    let mut changed = records;
    changed.endpoint.content.push(' ');
    assert_eq!(
        crate::pages::att::gateway_check(&changed, &release.workload, captured_at)["ok"],
        false
    );
}
