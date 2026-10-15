use super::*;
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::{RelaySigner, Tag};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const PARENT: &[u8] = include_bytes!("../../nostr/tests/fixtures/atif/parent.atif.jsonl");
const CHILD: &[u8] = include_bytes!("../../nostr/tests/fixtures/atif/child.atif.jsonl");
fn signer(n: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:02x}").repeat(32)).unwrap()
}
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn package() -> Package {
    prepare(PARENT, Form::Log, Options::default()).unwrap()
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn exact_log_bytes_and_rendered_digests_round_trip() {
    let package = package();
    let result = verify(&package.manifest, &package.chunks).unwrap();
    assert_eq!(result.bytes, PARENT);
    assert_eq!(result.status, Verification::Verified);
    assert_eq!(result.manifest.body()["schema_version"], "ATIF-v1.8");
    assert_eq!(result.manifest.body()["step_count"], 3);
    assert_eq!(
        result.manifest.body()["artifact"]["digest"],
        digest_bytes(PARENT)
    );
}
#[test]
fn public_sharing_is_explicit_and_binds_tags_author_and_pins() {
    let package = package();
    let alice = signer(1);
    assert!(carriage::public(&package, &alice, now(), false).is_err());
    let bundle = carriage::public(&package, &alice, now(), true).unwrap();
    assert_eq!(
        carriage::receive(&bundle.manifest, &bundle.chunks, None)
            .unwrap()
            .bytes,
        PARENT
    );
    assert_eq!(bundle.manifest.kind, 3198);
    assert_eq!(bundle.chunks[0].kind, 3199);
    let original = &bundle.chunks[0];
    let mut foreign = bundle.chunks.clone();
    foreign[0] = signer(3).sign(
        original.created_at,
        original.kind,
        original.tags.clone(),
        original.content.clone(),
    );
    assert!(carriage::receive(&bundle.manifest, &foreign, None).is_err());
    let mut changed = bundle.manifest.clone();
    changed.content.push(' ');
    assert!(carriage::receive(&changed, &bundle.chunks, None).is_err());
    let mut tags = bundle.manifest.tags.clone();
    tags[1] = Tag::new(vec!["x".into(), "ab".repeat(32)]);
    let forged = alice.sign(
        bundle.manifest.created_at,
        3198,
        tags,
        bundle.manifest.content.clone(),
    );
    assert!(carriage::receive(&forged, &bundle.chunks, None).is_err());
}
#[test]
fn private_sharing_encrypts_every_object_with_a_fresh_scope() {
    let package = package();
    let at = now();
    let one = carriage::private(&package, &secret(1), signer(2).pubkey(), at, at + 300).unwrap();
    let two = carriage::private(&package, &secret(1), signer(2).pubkey(), at, at + 300).unwrap();
    assert_eq!(one.manifest.kind, 3188);
    assert!(!one.manifest.content.contains("fix-readme"));
    assert!(!one.chunks[0].content.contains("README"));
    assert_eq!(
        carriage::receive(&one.manifest, &one.chunks, Some(&secret(2)))
            .unwrap()
            .bytes,
        PARENT
    );
    assert!(carriage::receive(&one.manifest, &one.chunks, None).is_err());
    assert!(carriage::receive(&one.manifest, &one.chunks, Some(&secret(3))).is_err());
    assert_ne!(one.manifest.tags[1], two.manifest.tags[1]);
    assert!(carriage::receive(&one.manifest, &two.chunks, Some(&secret(2))).is_err());
    assert!(carriage::private(&package, &secret(1), signer(2).pubkey(), at, at).is_err());
}
#[test]
fn missing_duplicate_and_changed_chunks_cannot_be_rendered_as_complete() {
    let package = package();
    assert!(
        verify(&package.manifest, &[])
            .unwrap_err()
            .contains("content_unavailable")
    );
    let mut duplicate = package.chunks.clone();
    duplicate.push(duplicate[0].clone());
    assert!(
        verify(&package.manifest, &duplicate)
            .unwrap_err()
            .contains("duplicate")
    );
    let mut changed = package.chunks.clone();
    changed[0]["text"] = json!("different bytes");
    assert!(verify(&package.manifest, &changed).is_err());
    let mut altered = package.manifest.body().clone();
    altered["step_count"] = json!(20);
    let altered = Manifest::parse(altered).unwrap();
    assert!(verify(&altered, &package.chunks).is_err());
    let mut altered = package.manifest.body().clone();
    altered["steps_digest"] = json!("cd".repeat(32));
    assert!(verify(&Manifest::parse(altered).unwrap(), &package.chunks).is_err());
}
#[test]
fn closed_shapes_versions_and_selector_pins_are_enforced() {
    let package = package();
    for (field, value) in [
        ("unexpected", json!(true)),
        ("requires", json!(["unsafe"])),
        ("schema_version", json!("ATIF-v2.0")),
        ("selector", json!({"set":"unfrozen","bank":null})),
    ] {
        let mut body = package.manifest.body().clone();
        body[field] = value;
        assert!(Manifest::parse(body).is_err(), "{field}");
    }
    let mut body = package.manifest.body().clone();
    body["selector"] = json!({"set":format!("tools@{}","aa".repeat(32)),"bank":null});
    Manifest::parse(body).unwrap();
    let mut body = package.manifest.body().clone();
    body.as_object_mut().unwrap().remove("state");
    assert!(Manifest::parse(body).is_err());
    let mut body = package.manifest.body().clone();
    body["artifact"]["digest"] = json!(format!("sha256:{}", "ZZ".repeat(32)));
    assert!(Manifest::parse(body).is_err());
}
#[test]
fn historical_recordings_keep_the_declared_version() {
    for version in atif::SUPPORTED_SCHEMA_VERSIONS {
        let text = std::str::from_utf8(PARENT)
            .unwrap()
            .replace("ATIF-v1.8", version);
        let package = prepare(text.as_bytes(), Form::Log, Options::default()).unwrap();
        assert_eq!(package.manifest.body()["schema_version"], version);
        assert_eq!(
            verify(&package.manifest, &package.chunks).unwrap().bytes,
            text.as_bytes()
        );
    }
}
#[test]
fn incomplete_and_redacted_recordings_remain_partial() {
    let lines = std::str::from_utf8(PARENT).unwrap();
    let prefix = &lines[..lines.rfind("{\"record\":\"end\"").unwrap()];
    let package = prepare(prefix.as_bytes(), Form::Log, Options::default()).unwrap();
    assert_eq!(
        verify(&package.manifest, &package.chunks).unwrap().status,
        Verification::Partial
    );
    let changed = prepare(
        PARENT,
        Form::Log,
        Options {
            derivation: Some(json!({"kind":"redacted","source":null})),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(changed.manifest.body()["derivation"]["source"], Value::Null);
    assert_eq!(
        verify(&changed.manifest, &changed.chunks).unwrap().status,
        Verification::Partial
    );
    let mut body = changed.manifest.body().clone();
    body["coverage"] = json!("complete");
    assert!(Manifest::parse(body).is_err());
}
#[test]
fn large_unicode_and_escaped_records_fit_private_payloads() {
    for message in ["界".repeat(40_000), "\\\"\u{0001}".repeat(30_000)] {
        let mut records = std::str::from_utf8(PARENT)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .collect::<Vec<_>>();
        records[1]["step"]["message"] = json!(message);
        let bytes = records.iter().map(|v| format!("{v}\n")).collect::<String>();
        let package = prepare(bytes.as_bytes(), Form::Log, Options::default()).unwrap();
        assert!(package.chunks.len() > 1);
        assert!(
            package
                .chunks
                .iter()
                .all(|v| v["text"].as_str().unwrap().len() <= MAX_TEXT
                    && jcs(v).unwrap().len() <= 65_535)
        );
        let reversed = package.chunks.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(
            verify(&package.manifest, &reversed).unwrap().bytes,
            bytes.as_bytes()
        );
        let at = now();
        let private =
            carriage::private(&package, &secret(1), signer(2).pubkey(), at, at + 300).unwrap();
        assert_eq!(
            carriage::receive(&private.manifest, &private.chunks, Some(&secret(2)))
                .unwrap()
                .bytes,
            bytes.as_bytes()
        );
    }
}
#[test]
fn checked_in_parent_child_and_continuation_links_pin_both_sides() {
    let parent = Manifest::parse(
        serde_json::from_slice(include_bytes!(
            "../../nostr/tests/fixtures/atif/parent.manifest.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let parent = verify(
        &parent,
        &[
            serde_json::from_slice(include_bytes!(
                "../../nostr/tests/fixtures/atif/parent.chunk-0.json"
            ))
            .unwrap(),
            serde_json::from_slice(include_bytes!(
                "../../nostr/tests/fixtures/atif/parent.chunk-1.json"
            ))
            .unwrap(),
        ],
    )
    .unwrap();
    let child = Manifest::parse(
        serde_json::from_slice(include_bytes!(
            "../../nostr/tests/fixtures/atif/child.manifest.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut child = verify(
        &child,
        &[serde_json::from_slice(include_bytes!(
            "../../nostr/tests/fixtures/atif/child.chunk-0.json"
        ))
        .unwrap()],
    )
    .unwrap();
    check_child(&parent, &child).unwrap();
    assert_eq!(child.bytes, CHILD);
    let mut value = child.manifest.body().clone();
    value["parent"]["step_id"] = json!(1);
    child.manifest = Manifest::parse(value).unwrap();
    assert!(check_child(&parent, &child).is_err());
    let mut current = parent.clone();
    let mut previous = parent.clone();
    let mut old = previous.manifest.body().clone();
    old["trajectory_id"] = json!("previous");
    previous.manifest = Manifest::parse(old).unwrap();
    let mut value = current.manifest.body().clone();
    value["previous"] =
        json!({"trajectory_id":"previous","steps_digest":previous.manifest.body()["steps_digest"]});
    current.manifest = Manifest::parse(value).unwrap();
    check_previous(&current, &previous).unwrap();
    assert!(check_previous(&current, &current).is_err());
}
#[test]
fn document_derivatives_have_distinct_byte_identity_and_partial_coverage() {
    let original = verify(&package().manifest, &package().chunks).unwrap();
    let bytes = serde_json::to_vec(&original.document).unwrap();
    let package = prepare(
        &bytes,
        Form::Document,
        Options {
            derivation: Some(json!({"kind":"converted","source":null})),
            ..Default::default()
        },
    )
    .unwrap();
    assert_ne!(
        package.manifest.body()["artifact"]["digest"],
        original.manifest.body()["artifact"]["digest"]
    );
    assert_eq!(
        package.manifest.body()["steps_digest"],
        original.manifest.body()["steps_digest"]
    );
    assert_eq!(
        verify(&package.manifest, &package.chunks).unwrap().status,
        Verification::Partial
    );
}

#[tokio::test]
async fn public_and_private_relay_exchange_survives_reconnect_and_exact_retry() {
    let (url, hub) = pylon::fixture::relay().await.unwrap();
    let alice = signer(1);
    let at = now();
    let package = package();
    let public = carriage::public(&package, &alice, at, true).unwrap();
    relay::publish(&url, &secret(1), &public).await.unwrap();
    let got = relay::fetch(&url, &secret(3), &public.manifest.id, &BTreeSet::new())
        .await
        .unwrap();
    assert_eq!(got.bytes, PARENT);
    let private =
        carriage::private(&package, &secret(1), signer(2).pubkey(), at, at + 300).unwrap();
    relay::publish(&url, &secret(1), &private).await.unwrap();
    relay::publish(&url, &secret(1), &private).await.unwrap();
    let got = relay::fetch(&url, &secret(2), &private.manifest.id, &BTreeSet::new())
        .await
        .unwrap();
    assert_eq!(got.bytes, PARENT);
    assert!(
        relay::fetch(&url, &secret(3), &private.manifest.id, &BTreeSet::new())
            .await
            .is_err()
    );
    let stored = &hub.lock().await.stored;
    // This fixture retains duplicate publications. Retrieval above must still
    // produce one verified result, while its raw-frame budget stays bounded.
    assert!(
        stored.iter().position(|e| e.id == private.chunks[0].id)
            < stored.iter().position(|e| e.id == private.manifest.id)
    );
}

#[tokio::test]
async fn source_only_declarations_never_fetch_unapproved_or_plain_http_hosts() {
    let (url, _) = pylon::fixture::relay().await.unwrap();
    for source in [
        "https://unapproved.invalid/trace.jsonl",
        "http://127.0.0.1:9/trace.jsonl",
    ] {
        let mut body = package().manifest.body().clone();
        body["chunks"] = json!([]);
        body["artifact"]["sources"] = json!([{"url":source}]);
        let package = Package {
            manifest: Manifest::parse(body).unwrap(),
            chunks: Vec::new(),
        };
        let bundle = carriage::public(&package, &signer(1), now(), true).unwrap();
        relay::publish(&url, &secret(1), &bundle).await.unwrap();
        let hosts = BTreeSet::from(["127.0.0.1".into()]);
        assert!(
            relay::fetch(&url, &secret(2), &bundle.manifest.id, &hosts)
                .await
                .unwrap_err()
                .contains("no permitted trajectory source")
        );
    }
}

#[tokio::test]
async fn private_mailbox_retrieval_supports_manifests_without_event_hints() {
    let (url, _) = pylon::fixture::relay().await.unwrap();
    let at = now();
    let bundle =
        carriage::private(&package(), &secret(1), signer(2).pubkey(), at, at + 300).unwrap();
    let opened = nostr::private_artifact::open(&bundle.manifest, &secret(1)).unwrap();
    let mut inline: Value = serde_json::from_slice(opened.inline_bytes().unwrap()).unwrap();
    for chunk in inline["chunks"].as_array_mut().unwrap() {
        chunk.as_object_mut().unwrap().remove("event");
    }
    let bytes = jcs(&inline).unwrap();
    let envelope = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":MANIFEST_SCHEMA},"inline":inline,"issued_at":at,"retain_until":at+300});
    let recipient = signer(2).pubkey().parse().unwrap();
    let manifest = nostr::private_artifact::seal(
        &envelope,
        &secret(1),
        &recipient,
        bundle.manifest.tags[1].value().unwrap(),
        at,
        secp256k1::rand::random(),
    )
    .unwrap();
    let bundle = carriage::Bundle {
        manifest,
        chunks: bundle.chunks,
    };
    relay::publish(&url, &secret(1), &bundle).await.unwrap();
    assert_eq!(
        relay::fetch(&url, &secret(2), &bundle.manifest.id, &BTreeSet::new())
            .await
            .unwrap()
            .bytes,
        PARENT
    );
}
