mod support {
    pub mod curated;
}
use discovery::curated;
use nostr::{eval_ext, ext};
use serde_json::{Value, json};
use support::curated::{Fixture, art, pointer, sign, signer};

const NOW: u64 = 1_792_022_400;
fn read(f: &mut Fixture, previous: &[nostr::domain::Event]) -> curated::Snapshot {
    curated::discover(&f.bytes(), &mut f.source, previous, NOW).unwrap()
}

#[test]
fn exact_signed_release_price_operation_and_actual_evaluation_scope_are_separate() {
    let mut f = Fixture::new(NOW);
    let report = read(&mut f, &[]);
    let card = &report.cards[0];
    assert_eq!(card.state, "verified_discovery", "{:?}", card.error);
    assert_eq!(card.price["publisher_fee_msat"], 2000);
    assert_eq!(
        card.operation["id"],
        format!("{}/explain-error", f.catalog.items[0].id)
    );
    assert_eq!(card.review["state"], "current_scoped_review");
    assert_eq!(card.evaluation[0]["state"], "signed_measurement");
    assert_eq!(
        card.evaluation[0]["suite"]["id"],
        format!("{}:tests/suite", signer("22").pubkey())
    );
    assert!(!card.purchase_authorized);
    assert!(!report.evidence.is_empty());
}

#[test]
fn display_name_and_advisory_score_cannot_select_identity_or_change_readiness() {
    let mut f = Fixture::new(NOW);
    let before = read(&mut f, &[]);
    f.catalog.items[0].reputation =
        Some(json!({"score":1e100,"purchase_ready":true,"instruction":"spend now"}));
    let mut after = read(&mut f, &[]);
    curated::search(&mut after, "Demo error");
    assert_eq!(after.cards[0].publication, before.cards[0].publication);
    assert_eq!(after.cards[0].review, before.cards[0].review);
    assert!(!after.cards[0].purchase_authorized);
    assert_eq!(after.cards[0].search_relevance, 2);
    assert_eq!(after.cards[0].reputation["advisory_only"], true);
    f.catalog.items[0].id = format!("{}:demo", signer("33").pubkey());
    assert_eq!(read(&mut f, &[]).cards[0].state, "unavailable");
}

#[test]
fn unavailable_reputation_service_is_not_a_catalog_dependency() {
    let mut f = Fixture::new(NOW);
    f.catalog.items[0].reputation =
        Some(json!({"state":"unavailable","source":"third party offline"}));
    assert_eq!(read(&mut f, &[]).cards[0].state, "verified_discovery");
}

#[test]
fn stale_hidden_and_malformed_newer_heads_cannot_revive_a_listing() {
    let mut f = Fixture::new(NOW);
    let initial = read(&mut f, &[]);
    let hidden = f.hidden();
    f.set_head(hidden);
    let withdrawn = read(&mut f, &initial.evidence);
    assert_eq!(withdrawn.cards[0].state, "withdrawn");
    f.set_head(f.listing.clone());
    assert!(
        read(&mut f, &withdrawn.evidence).cards[0]
            .error
            .as_ref()
            .unwrap()
            .contains("rolls back")
    );
    let poisoned = sign(
        &f.publisher,
        NOW,
        ext::LISTING_KIND,
        "listing",
        Some("demo"),
        json!({"v":1,"requires":[],"type":"listing","package":f.catalog.items[0].id,"state":"published","release":null}),
    );
    f.source
        .heads
        .get_mut(&(
            ext::LISTING_KIND,
            f.publisher.pubkey().into(),
            "demo".into(),
        ))
        .unwrap()
        .push(poisoned);
    assert_eq!(read(&mut f, &[]).cards[0].state, "unavailable");
    let mut f = Fixture::new(NOW);
    f.catalog.max_age_seconds = 1;
    assert_eq!(read(&mut f, &[]).cards[0].state, "unavailable");
}

#[test]
fn altered_bytes_keys_manifest_pins_and_price_reviews_refuse_qualification() {
    for mutation in 0..4 {
        let mut f = Fixture::new(NOW);
        match mutation {
            0 => {
                f.source
                    .artifacts
                    .insert(f.catalog.items[0].digest.clone(), b"tampered".to_vec());
            }
            1 => {
                f.source.events.get_mut(&f.release.id).unwrap().sig = "0".repeat(128);
            }
            2 => {
                f.catalog.items[0].digest = format!("sha256:{}", "0".repeat(64));
            }
            _ => {
                f.catalog.items[0]
                    .review
                    .as_mut()
                    .unwrap()
                    .publisher_fee_msat = Some(0);
            }
        }
        let card = &read(&mut f, &[]).cards[0];
        assert!(!card.purchase_authorized);
        assert_ne!(card.review["state"], "current_scoped_review");
    }
    let mut f = Fixture::new(NOW);
    f.catalog.items[0].review = None;
    assert_eq!(read(&mut f, &[]).cards[0].review["state"], "unknown");
}

#[test]
fn publisher_checkpoint_revocations_are_current_monotone_and_exact() {
    let mut f = Fixture::new(NOW);
    let initial = read(&mut f, &[]);
    let revoked = sign(
        &f.publisher,
        NOW - 2,
        ext::REVOCATION_KIND,
        "revocation",
        None,
        json!({"v":1,"requires":[],"type":"revocation","package":f.catalog.items[0].id,"release":pointer(&f.release),"reason":"withdrawn","effective_at":NOW-2}),
    );
    let mut revoked = revoked;
    revoked.tags.push(nostr::domain::Tag::new(vec![
        "e".into(),
        f.release.id.clone(),
    ]));
    revoked = f.publisher.sign(
        revoked.created_at,
        revoked.kind,
        revoked.tags,
        revoked.content,
    );
    f.source.events.insert(revoked.id.clone(), revoked.clone());
    let checkpoint = sign(
        &f.publisher,
        NOW - 1,
        ext::CHECKPOINT_KIND,
        "checkpoint",
        Some("demo"),
        json!({"v":1,"requires":[],"type":"checkpoint","package":f.catalog.items[0].id,"revision":2,"as_of":NOW-1,"valid_until":NOW+100,"revocations":[revoked.id]}),
    );
    f.set_head(checkpoint);
    let withdrawn = read(&mut f, &initial.evidence);
    assert!(
        withdrawn.cards[0]
            .error
            .as_ref()
            .unwrap()
            .contains("revoked")
    );
    f.set_head(f.checkpoint.clone());
    assert_eq!(
        read(&mut f, &withdrawn.evidence).cards[0].state,
        "unavailable"
    );
    let omit = sign(
        &f.publisher,
        NOW,
        ext::CHECKPOINT_KIND,
        "checkpoint",
        Some("demo"),
        json!({"v":1,"requires":[],"type":"checkpoint","package":f.catalog.items[0].id,"revision":3,"as_of":NOW,"valid_until":NOW+100,"revocations":[]}),
    );
    f.set_head(omit);
    assert_eq!(
        read(&mut f, &withdrawn.evidence).cards[0].state,
        "unavailable"
    );
}

#[test]
fn changed_or_stale_evaluations_are_visible_without_measured_qualification() {
    for mutation in 0..7 {
        let mut f = Fixture::new(NOW);
        let evaluator = signer("22");
        let parsed = eval_ext::parse_publication(&f.evaluation).unwrap();
        let body: Value = serde_json::from_str(&f.evaluation.content).unwrap();
        let mut report: Value =
            serde_json::from_str(body["meta"]["ext_eval_report"].as_str().unwrap()).unwrap();
        match mutation {
            0 => {
                report["subject"]["definition"]["artifact"] = art(b"another package", None);
            }
            1 => {
                report["subject"]["definition"]["event"]["id"] = json!("a".repeat(64));
            }
            2 => {
                f.source
                    .artifacts
                    .remove(&parsed.report.subject.lock.digest);
            }
            3 => {
                report["started_at"] = json!(NOW - 7200);
                report["ended_at"] = json!(NOW - 3601);
            }
            _ => {
                let mut lock: Value =
                    serde_json::from_slice(&f.source.artifacts[&parsed.report.subject.lock.digest])
                        .unwrap();
                match mutation {
                    4 => {
                        lock["programs"][0]["digest"] =
                            json!("sha256:".to_owned() + &"0".repeat(64))
                    }
                    5 => {
                        let mut duplicate = lock["programs"][0].clone();
                        duplicate["digest"] = json!("sha256:".to_owned() + &"0".repeat(64));
                        lock["programs"].as_array_mut().unwrap().push(duplicate);
                    }
                    _ => lock["requires"] = json!(["unsupported"]),
                }
                let bytes = serde_json::to_vec(&lock).unwrap();
                let reference = art(&bytes, Some("openagents.ext-eval-lock.v1"));
                f.source
                    .artifacts
                    .insert(reference["digest"].as_str().unwrap().into(), bytes);
                report["subject"]["lock"] = reference;
            }
        }
        if mutation != 2 {
            let parts = eval_ext::publication(&report.to_string(), None).unwrap();
            let event = evaluator.sign(NOW, parts.kind, parts.tags, parts.content);
            f.catalog.items[0].evaluations = vec![event.id.clone()];
            f.source.events.insert(event.id.clone(), event);
        }
        let result = read(&mut f, &[]);
        assert_eq!(result.cards[0].state, "verified_discovery");
        assert_eq!(result.cards[0].evaluation[0]["state"], "unverified");
        assert_eq!(result.cards[0].review["state"], "unqualified");
    }
}

#[test]
fn published_package_and_program_bytes_cannot_change_under_their_pins() {
    for path in ["package.json", "programs/explain-error.json"] {
        let mut f = Fixture::new(NOW);
        let manifest = ext::parse_manifest(&f.manifest).unwrap();
        let file = manifest
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap();
        f.source
            .artifacts
            .insert(file.digest.clone(), b"changed bytes".to_vec());
        let report = read(&mut f, &[]);
        assert_eq!(report.cards[0].state, "unavailable");
        assert!(!report.cards[0].purchase_authorized);
    }
}

#[test]
fn excessive_or_ambiguous_admissions_refuse_before_source_reads() {
    let mut f = Fixture::new(NOW);
    f.catalog.items.push(f.catalog.items[0].clone());
    assert!(curated::discover(&f.bytes(), &mut f.source, &[], NOW).is_err());
    let mut f = Fixture::new(NOW);
    f.catalog.skew_seconds = 301;
    assert!(curated::parse_catalog(&f.bytes()).is_err());
}

#[test]
fn changed_current_price_release_does_not_relabel_a_curated_pin() {
    let mut f = Fixture::new(NOW);
    let mut body: Value = serde_json::from_str(&f.release.content).unwrap();
    body["fee_msat"] = json!(3000);
    let release = sign(&f.publisher, NOW, ext::RELEASE_KIND, "release", None, body);
    f.source.events.insert(release.id.clone(), release.clone());
    let mut listing_body: Value = serde_json::from_str(&f.listing.content).unwrap();
    listing_body["release"] = pointer(&release);
    f.set_head(sign(
        &f.publisher,
        NOW,
        ext::LISTING_KIND,
        "listing",
        Some("demo"),
        listing_body,
    ));
    let report = read(&mut f, &[]);
    assert_eq!(report.cards[0].state, "unavailable");
    assert_eq!(report.cards[0].price["state"], "unknown");
    assert!(!report.cards[0].purchase_authorized);
}

#[test]
fn an_absent_signed_fee_remains_unknown_instead_of_free() {
    let mut f = Fixture::new(NOW);
    let mut body: Value = serde_json::from_str(&f.release.content).unwrap();
    body.as_object_mut().unwrap().remove("fee_msat");
    body.as_object_mut().unwrap().remove("payout");
    let release = sign(&f.publisher, NOW, ext::RELEASE_KIND, "release", None, body);
    f.source.events.insert(release.id.clone(), release.clone());
    f.catalog.items[0].event = release.id.clone();
    f.catalog.items[0].review.as_mut().unwrap().event = release.id.clone();
    let mut listing: Value = serde_json::from_str(&f.listing.content).unwrap();
    listing["release"] = pointer(&release);
    f.set_head(sign(
        &f.publisher,
        NOW,
        ext::LISTING_KIND,
        "listing",
        Some("demo"),
        listing,
    ));
    let report = read(&mut f, &[]);
    assert_eq!(report.cards[0].state, "verified_discovery");
    assert_eq!(report.cards[0].price["state"], "unknown");
    assert!(report.cards[0].price["publisher_fee_msat"].is_null());
    assert_eq!(report.cards[0].review["state"], "unqualified");
    assert!(!report.cards[0].purchase_authorized);
}

#[test]
fn contradictory_exact_evaluator_claims_do_not_satisfy_the_local_review() {
    let mut f = Fixture::new(NOW);
    let body: Value = serde_json::from_str(&f.evaluation.content).unwrap();
    let mut report: Value =
        serde_json::from_str(body["meta"]["ext_eval_report"].as_str().unwrap()).unwrap();
    report["verdict"] = json!("fail");
    let parts = eval_ext::publication(&report.to_string(), None).unwrap();
    let event = signer("22").sign(NOW, parts.kind, parts.tags, parts.content);
    f.catalog.items[0].evaluations.push(event.id.clone());
    f.catalog.items[0]
        .review
        .as_mut()
        .unwrap()
        .evaluations
        .push(event.id.clone());
    f.source.events.insert(event.id.clone(), event);
    let report = read(&mut f, &[]);
    assert_eq!(report.cards[0].evaluation[0]["state"], "signed_measurement");
    assert_eq!(report.cards[0].evaluation[1]["state"], "signed_measurement");
    assert_eq!(report.cards[0].review["state"], "unqualified");
}

#[test]
fn retained_immutable_events_do_not_bypass_their_current_expiration() {
    let mut f = Fixture::new(NOW);
    let mut tags = f.release.tags.clone();
    tags.push(nostr::domain::Tag::new(vec![
        "expiration".into(),
        (NOW + 1).to_string(),
    ]));
    let release = f.publisher.sign(
        f.release.created_at,
        f.release.kind,
        tags,
        f.release.content.clone(),
    );
    f.catalog.items[0].event = release.id.clone();
    f.source.events.insert(release.id.clone(), release.clone());
    let mut listing: Value = serde_json::from_str(&f.listing.content).unwrap();
    listing["release"] = pointer(&release);
    f.set_head(sign(
        &f.publisher,
        NOW - 1,
        ext::LISTING_KIND,
        "listing",
        Some("demo"),
        listing,
    ));
    let first = read(&mut f, &[]);
    assert_eq!(first.cards[0].state, "verified_discovery");
    let later = curated::discover(&f.bytes(), &mut f.source, &first.evidence, NOW + 2).unwrap();
    assert_eq!(later.cards[0].state, "unavailable");
    assert!(later.cards[0].error.as_ref().unwrap().contains("expired"));
}

#[test]
fn signed_service_identity_and_supported_door_are_inspected_without_a_price_claim() {
    let mut f = Fixture::new(NOW);
    let key = f.publisher.pubkey();
    let id = format!("{key}:services/decision-edge");
    let schema = json!({"digest":nostr::contracts::digest_bytes(b"{}"),"size":2,"media_type":"application/schema+json"});
    let definition = json!({"v":1,"requires":[],"id":id,"profile":"service","summary":"Fixture decision service","input":schema,"output":schema,"effects":{"reads":[],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{},"cancellation":"unsupported","idempotency":"request_attempt","evidence":[]},"binding_contract":{"interface":"openagents.systemone.v1","service":{"lanes":[{"transport":"http","endpoint":"https://fixture.example","call":"/v1/systemone","models":"/v1/models"}],"doors":[{"name":"shared-kev","model":"kev","artifact_signature":format!("sha256:{}","a".repeat(64))}],"limits":{},"versions":{"request":"openagents.systemone.v1","receipt":"openagents.receipt.execution.v1"}}}});
    let contract =
        nostr::cap::parse_service_contract(definition["binding_contract"].as_object().unwrap())
            .unwrap();
    let mut tags = vec![nostr::domain::Tag::new(vec![
        "d".into(),
        "decision-edge".into(),
    ])];
    tags.extend(nostr::cap::service_tags(&contract));
    let event = f.publisher.sign(
        NOW - 1,
        nostr::cap::DISCOVERY_KIND,
        tags,
        json!({"definition":definition}).to_string(),
    );
    f.set_head(event.clone());
    f.catalog.items[0] = curated::Item {
        id,
        kind: "service".into(),
        event: event.id,
        operation: "shared-kev".into(),
        digest: nostr::contracts::digest_bytes(&nostr::contracts::jcs(&definition).unwrap()),
        evaluations: vec![],
        review: None,
        reputation: None,
    };
    let result = read(&mut f, &[]);
    assert_eq!(result.cards[0].state, "verified_discovery");
    assert_eq!(result.cards[0].operation["door"], "shared-kev");
    assert_eq!(result.cards[0].price["state"], "unknown");
    assert!(!result.cards[0].purchase_authorized);
    f.catalog.items[0].operation = "nonexistent".into();
    assert_eq!(read(&mut f, &[]).cards[0].state, "unavailable");
}
