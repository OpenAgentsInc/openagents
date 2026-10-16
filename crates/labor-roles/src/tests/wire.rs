use super::*;
use crate::wire::{Admission, Bytes};
use serde_json::{Value, json};
fn put(bytes: &mut Bytes, value: &Value) -> Value {
    let raw = nostr::contracts::jcs(value).unwrap();
    let digest = nostr::contracts::digest_bytes(&raw);
    bytes.insert(digest.clone(), raw.clone());
    json!({"digest":digest,"size":raw.len(),"media_type":"application/json","schema":value["v"]})
}
fn sealed(body: &Value, who: u8, now: u64) -> nostr::private_artifact::OpenEnvelope {
    let raw = nostr::contracts::jcs(body).unwrap();
    let reference = json!({"digest":nostr::contracts::digest_bytes(&raw),"size":raw.len(),"media_type":"application/json","schema":body["v"]});
    let envelope = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference,"inline":body,"issued_at":now,"retain_until":NOW+100000});
    let event = nostr::private_artifact::seal(
        &envelope,
        &secret(who),
        &key(BUYER).parse().unwrap(),
        &"ba".repeat(32),
        now,
        [3; 32],
    )
    .unwrap();
    nostr::private_artifact::open(&event, &secret(BUYER)).unwrap()
}
#[test]
fn canonical_wire_references_join_the_role_book_without_rehashing_its_model() {
    let t = terms(1);
    let mut bytes = Bytes::new();
    let market_record = put(
        &mut bytes,
        &json!({"v":nostr::market_contracts::RECORD_SCHEMA}),
    );
    let order = json!({"market":t.market,"order_id":t.order,"buyer":t.buyer,"provider":t.provider,"order":market_record,"confirmation":market_record});
    let policy = put(
        &mut bytes,
        &json!({"v":"openagents.labor-acceptance-policy.v1"}),
    );
    // This unit test starts from a locally admitted order. The production
    // constructor additionally requires the confirmed MKT negotiation.
    let wire = Admission::fixture(
        order.clone(),
        policy.clone(),
        BTreeMap::from([(
            "patch".into(),
            json!({"type":"object","required":["patch"],"properties":{"patch":{"type":"string"}},"additionalProperties":false}),
        )]),
    );
    let item = put(
        &mut bytes,
        &json!({"v":"patch.json","patch":"actual UTF-8 content"}),
    );
    // The schema's additionalProperties rule deliberately rejects this body.
    let link = put(&mut bytes, &json!({"v":"openagents.labor-execution.v1"}));
    let run = put(&mut bytes, &json!({"v":nostr::run::RECORD_SCHEMA}));
    let inert = put(
        &mut bytes,
        &json!({"v":"inert.text.v1","text":"no execution claim in this wire conversion test"}),
    );
    let mut submission = json!({"v":SUBMISSION_V,"requires":[],"issuer":key(WORKER),"order":order,"number":0,"previous":null,"rework":null,
        "executions":[link],"deliverables":[{"id":"patch","content":item}],"run_evidence":[run],"limitations":inert});
    let mut book = crate::book::Book::new(t, NOW).unwrap();
    assert!(
        wire.ingest(
            &mut book,
            &sealed(&submission, WORKER, NOW),
            &key(BUYER),
            &bytes,
            NOW
        )
        .is_err()
    );
    // Complete JSON delivery bytes are checked against the pinned schema.
    let raw = nostr::contracts::jcs(&json!({"patch":"actual UTF-8 content"})).unwrap();
    let digest = nostr::contracts::digest_bytes(&raw);
    bytes.insert(digest.clone(), raw.clone());
    submission["deliverables"][0]["content"] =
        json!({"digest":digest,"size":raw.len(),"media_type":"application/json"});
    let original = sealed(&submission, WORKER, NOW);
    let canonical = original.artifact().digest.clone();
    assert_eq!(
        wire.ingest(&mut book, &original, &key(BUYER), &bytes, NOW)
            .unwrap(),
        Applied::New(canonical.clone())
    );
    assert_eq!(book.entries()[0].digest, canonical);
    assert_ne!(
        crate::digest_of(&book.entries()[0].record).unwrap(),
        canonical
    );
    assert!(
        wire.ingest(
            &mut book,
            &sealed(&submission, PROVIDER, NOW),
            &key(BUYER),
            &bytes,
            NOW
        )
        .is_err()
    );
    assert_eq!(
        wire.ingest(&mut book, &original, &key(BUYER), &bytes, NOW)
            .unwrap(),
        Applied::Duplicate(canonical.clone())
    );
    let sub_ref = put(&mut bytes, &submission);
    let delivery = json!({"v":DELIVERY_V,"requires":[],"issuer":key(BUYER),"order":order,"submission":sub_ref,"received_at":NOW+1,"available":true});
    assert!(matches!(
        wire.ingest(
            &mut book,
            &sealed(&delivery, BUYER, NOW + 1),
            &key(BUYER),
            &bytes,
            NOW + 1
        )
        .unwrap(),
        Applied::New(_)
    ));
    assert_eq!(book.entries().len(), 2);
    assert_eq!(book.original(&canonical), Some(&submission));
    let checker = put(&mut bytes, &json!({"v":"checker.receipt.v1"}));
    let verification = json!({"v":VERIFICATION_V,"requires":[],"issuer":key(REVIEWER),"order":order,"submission":sub_ref,"policy":policy,
        "checker_receipts":[checker],"criteria":[{"id":"tests-pass","verdict":"passed","evidence":[inert]},
            {"id":"returns-42","verdict":"failed","evidence":[inert]}],"verdict":"failed","limitations":inert});
    wire.ingest(
        &mut book,
        &sealed(&verification, REVIEWER, NOW + 2),
        &key(BUYER),
        &bytes,
        NOW + 2,
    )
    .unwrap();
    let verification_ref = put(&mut bytes, &verification);
    let review = json!({"v":REVIEW_V,"requires":[],"issuer":key(BUYER),"order":order,"submission":sub_ref,"verification":verification_ref,
        "decision":"request_rework","criteria":["returns-42"],"reason":inert});
    let review_ref = put(&mut bytes, &review);
    wire.ingest(
        &mut book,
        &sealed(&review, BUYER, NOW + 3),
        &key(BUYER),
        &bytes,
        NOW + 3,
    )
    .unwrap();
    let review_digest = review_ref["digest"].as_str().unwrap();
    assert_eq!(book.original(review_digest), Some(&review));
    let task = "79".repeat(32);
    let initial = json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":key(WORKER),"policy":policy,
        "entries":[],"omissions":[],"coverage":"complete"});
    let mut descriptor = json!({"v":"openagents.evidence.v1","requires":[],"content":review_ref,
        "source":{"kind":"document","identity":book.terms.order,"version":review_digest,"observation":review_ref},
        "capture":{"complete":true,"omitted_bytes":0},"derived_from":[],"scope":{"task":task,"recipients":[key(WORKER)],"classification":"private"}});
    let evidence_id = crate::digest_of(&descriptor).unwrap();
    descriptor["id"] = json!(evidence_id);
    let mut proposed = initial.clone();
    proposed["entries"] =
        json!([{"evidence":evidence_id,"representation":review_ref,"mandatory":true}]);
    crate::context::admit_rework(
        &book,
        1,
        review_digest,
        &initial,
        &proposed,
        &BTreeMap::from([(evidence_id, descriptor)]),
        &bytes,
        NOW + 4,
    )
    .unwrap();

    let mut wrong = delivery.clone();
    wrong["order"]["order_id"] = json!("ff".repeat(32));
    assert!(
        wire.ingest(
            &mut book,
            &sealed(&wrong, BUYER, NOW + 1),
            &key(BUYER),
            &bytes,
            NOW + 1
        )
        .is_err()
    );
    wrong = delivery;
    wrong["requires"] = json!(["unknown"]);
    assert!(
        wire.ingest(
            &mut book,
            &sealed(&wrong, BUYER, NOW + 1),
            &key(BUYER),
            &bytes,
            NOW + 1
        )
        .is_err()
    );
    let mut corrupt = bytes.clone();
    corrupt.get_mut(&digest).unwrap().push(0);
    assert!(
        wire.ingest(&mut book, &original, &key(BUYER), &corrupt, NOW)
            .is_err()
    );
}

#[test]
fn canonical_participants_persist_before_sending_and_replay_original_signed_records() {
    let t = terms(1);
    let mut bytes = Bytes::new();
    let market = put(
        &mut bytes,
        &json!({"v":nostr::market_contracts::RECORD_SCHEMA}),
    );
    let order = json!({"market":t.market,"order_id":t.order,"buyer":t.buyer,"provider":t.provider,"order":market,"confirmation":market});
    let policy = put(
        &mut bytes,
        &json!({"v":"openagents.labor-acceptance-policy.v1"}),
    );
    let admission = Admission::fixture(
        order.clone(),
        policy,
        BTreeMap::from([("patch".into(), json!({"type":"object"}))]),
    );
    let output = put(
        &mut bytes,
        &json!({"v":"output.json","patch":"source change"}),
    );
    let execution = put(&mut bytes, &json!({"v":"openagents.labor-execution.v1"}));
    let run = put(&mut bytes, &json!({"v":nostr::run::RECORD_SCHEMA}));
    let inert = put(
        &mut bytes,
        &json!({"v":"inert.text.v1","text":"wire persistence test; no execution assertion"}),
    );
    let submission = json!({"v":SUBMISSION_V,"requires":[],"issuer":t.worker,"order":order,"number":0,"previous":null,"rework":null,
        "executions":[execution],"deliverables":[{"id":"patch","content":output}],"run_evidence":[run],"limitations":inert});
    let dirs = tempfile::tempdir().unwrap();
    let mut worker = crate::wire_host::Host::open(
        dirs.path().join("worker"),
        secret(WORKER),
        t.clone(),
        admission.clone(),
        NOW,
    )
    .unwrap();
    let mut buyer = crate::wire_host::Host::open(
        dirs.path().join("buyer"),
        secret(BUYER),
        t.clone(),
        admission.clone(),
        NOW,
    )
    .unwrap();
    assert!(
        crate::wire_host::Host::open(
            dirs.path().join("worker"),
            secret(WORKER),
            t.clone(),
            admission.clone(),
            NOW
        )
        .is_err()
    );
    let discarded = worker.issue(&submission, &bytes, NOW + 1).unwrap();
    let digest = crate::digest_of(&submission).unwrap();
    assert_eq!(worker.book().entries()[0].digest, digest);
    drop(discarded);
    drop(worker);
    let mut worker = crate::wire_host::Host::open(
        dirs.path().join("worker"),
        secret(WORKER),
        t.clone(),
        admission.clone(),
        NOW + 2,
    )
    .unwrap();
    assert_eq!(worker.book().original(&digest), Some(&submission));
    let resent = worker.issue(&submission, &bytes, NOW + 2).unwrap();
    assert_eq!(worker.book().entries().len(), 1);
    let event = resent
        .iter()
        .find(|e| e.tag_values("p").next() == Some(key(BUYER).as_str()))
        .unwrap();
    assert!(matches!(
        buyer.receive(event, &bytes, NOW + 2).unwrap(),
        Applied::New(_)
    ));
    assert!(matches!(
        buyer.receive(event, &bytes, NOW + 2).unwrap(),
        Applied::Duplicate(_)
    ));
    let sub_ref = put(&mut bytes, &submission);
    let delivery = json!({"v":DELIVERY_V,"requires":[],"issuer":t.buyer,"order":order,"submission":sub_ref,"received_at":NOW+2,"available":true});
    let outgoing = buyer.issue(&delivery, &bytes, NOW + 3).unwrap();
    let event = outgoing
        .iter()
        .find(|e| e.tag_values("p").next() == Some(key(WORKER).as_str()))
        .unwrap();
    worker.receive(event, &bytes, NOW + 3).unwrap();
    assert_eq!(worker.book().entries().len(), 2);
    let mut changed = submission;
    changed["number"] = json!(1);
    assert!(buyer.issue(&changed, &bytes, NOW + 4).is_err());
    drop(worker);
    std::fs::remove_file(dirs.path().join("worker/state.json")).unwrap();
    assert!(
        crate::wire_host::Host::open(
            dirs.path().join("worker"),
            secret(WORKER),
            t,
            admission,
            NOW + 5
        )
        .is_err()
    );
}
