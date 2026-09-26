use super::*;
use crate::domain::{RelaySigner, Tag};
use crate::private_artifact::{open, seal};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::json;

// Fixed keys and nonces are test-only fixtures, never wallet credentials.
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn public(n: u8) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::new(), &secret(n))
        .x_only_public_key()
        .0
}
fn signer(n: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:02x}").repeat(32)).unwrap()
}
fn reference(value: &Value, schema: &str) -> Value {
    let bytes = jcs(value).unwrap();
    json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema})
}
fn artifact_value(a: &ArtifactRef) -> Value {
    json!({"digest":a.digest,"size":a.size,"media_type":a.media_type,"schema":a.schema})
}
fn event_value(e: &EventRef) -> Value {
    json!({"id":e.id,"pubkey":e.pubkey,"kind":e.kind})
}
fn private(value: &Value, schema: &str, author: u8, recipient: u8, nonce: u8) -> OpenEnvelope {
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference(value,schema),"inline":value,"issued_at":100,"retain_until":1000});
    let event = seal(
        &body,
        &secret(author),
        &public(recipient),
        &format!("{nonce:02x}").repeat(32),
        110,
        [nonce; 32],
    )
    .unwrap();
    open(&event, &secret(recipient)).unwrap()
}
fn request() -> Value {
    json!({"v":"fixture.noop.v1","work":"none"})
}
fn capability() -> Value {
    json!({"id":format!("{}:fixture/noop",public(2)),"artifact":reference(&json!({"v":"fixture.cap.v1"}),"openagents.cap.v1")})
}
fn offer_body() -> Value {
    json!({"v":OFFERING_SCHEMA,"requires":[],"provider":public(2).to_string(),"offer":"noop","capability":capability(),"profiles":["fixture.noop.v1"],"payment_profiles":[FREE_PROFILE],"networks":[],"summary":"A no-spend fixture","price_hint_msat":0,"capacity_hint":1,"valid_until":900})
}
fn offering_event(value: &Value) -> Event {
    let content = String::from_utf8(jcs(value).unwrap()).unwrap();
    let digest = digest_bytes(content.as_bytes());
    signer(2).sign(
        100,
        OFFERING_KIND,
        vec![
            Tag::new(vec!["t".into(), "oa:market-offering:v1".into()]),
            Tag::new(vec!["x".into(), digest[7..].into()]),
        ],
        content,
    )
}
fn offering() -> Offering {
    parse_offering(&offering_event(&offer_body())).unwrap()
}
fn terms() -> Value {
    json!({"v":TERMS_SCHEMA,"requires":[],"profile":"fixture.noop.v1","profile_terms":reference(&request(),"fixture.noop.v1"),"buyer":public(1).to_string(),"provider":public(2).to_string(),"worker":public(3).to_string(),"price_msat":0,"fee_limit_msat":0,"payment_profile":FREE_PROFILE,"network":null,"quote_expires_at":200,"order_confirm_by":220,"delivery_due_at":300,"review_due_at":400,"payment_due_at":500,"retain_until":600})
}
fn document(value: &Value, author: u8) -> Result<TermsDocument, ContractError> {
    parse_terms_document(&private(value, TERMS_SCHEMA, author, 1, 4), None)
}
fn record_value(kind: &str, issuer: u8, seq: u64, prev: Option<&Record>, body: Value) -> Value {
    json!({"v":RECORD_SCHEMA,"requires":[],"type":kind,"market":"aa".repeat(32),"buyer":public(1).to_string(),"provider":public(2).to_string(),"issuer":public(issuer).to_string(),"seq":seq,"prev":prev.map(|p|artifact_value(p.artifact())),"issued_at":110,"body":body})
}
fn record(value: &Value, author: u8, nonce: u8) -> Result<Record, ContractError> {
    let recipient = if author == 1 { 2 } else { 1 };
    parse_record(
        &private(value, RECORD_SCHEMA, author, recipient, nonce),
        None,
    )
}
fn rfq() -> Record {
    record(&record_value("rfq",1,0,None,json!({"offering":event_value(offering().event()),"profile":"fixture.noop.v1","request":reference(&request(),"fixture.noop.v1"),"price_limit_msat":0,"response_due_at":190,"retain_until":600})),1,5).unwrap()
}
fn quote(rfq: &Record, terms: &Value, prev: Option<&Record>, id: &str) -> Record {
    record(&record_value("quote",2,prev.map_or(0,|r|r.seq+1),prev,json!({"rfq":artifact_value(rfq.artifact()),"quote_id":id,"terms":reference(terms,TERMS_SCHEMA)})),2,6).unwrap()
}
fn order(quote: &Record, prev: &Record, terms: &Value, id: &str) -> Record {
    record(&record_value("order",1,prev.seq+1,Some(prev),json!({"quote":artifact_value(quote.artifact()),"terms_digest":reference(terms,TERMS_SCHEMA)["digest"],"order_id":id})),1,7).unwrap()
}
fn ack(order: &Record, prev: &Record, confirmed: bool) -> Record {
    record(&record_value("order_ack",2,prev.seq+1,Some(prev),json!({"order":artifact_value(order.artifact()),"decision":if confirmed {"confirmed"} else {"refused"},"code":if confirmed {Value::Null} else {json!("unavailable")}})),2,8).unwrap()
}
struct Profile;
impl DomainProfile for Profile {
    fn id(&self) -> &str {
        "fixture.noop.v1"
    }
    fn validate_request(&self, proposed: &ArtifactRef) -> Result<(), ContractError> {
        let expected = parse_artifact(&reference(&request(), "fixture.noop.v1"))?;
        if same_artifact(proposed, &expected) {
            Ok(())
        } else {
            Err(identity("fixture request closure"))
        }
    }
    fn validate_terms(&self, terms: &Terms, proposed: &DefinitionRef) -> Result<(), ContractError> {
        self.validate_request(&terms.profile_terms)?;
        if proposed != &parse_definition(&capability())? {
            return Err(identity("fixture exact capability"));
        }
        Ok(())
    }
}
fn negotiation(max: usize) -> Negotiation {
    Negotiation::new(offering(), &public(1).to_string(), &"aa".repeat(32), max).unwrap()
}
fn through_order() -> (Negotiation, Record, Record, Record) {
    let mut n = negotiation(64);
    let r = rfq();
    let q = quote(&r, &terms(), None, &"bb".repeat(32));
    let o = order(&q, &r, &terms(), &"cc".repeat(32));
    assert_eq!(
        n.ingest(r.clone(), None, &Profile, 120, 0).unwrap(),
        Ingest::Applied
    );
    assert_eq!(
        n.ingest(
            q.clone(),
            Some(&document(&terms(), 2).unwrap()),
            &Profile,
            130,
            0
        )
        .unwrap(),
        Ingest::Applied
    );
    assert_eq!(
        n.ingest(o.clone(), None, &Profile, 140, 0).unwrap(),
        Ingest::Applied
    );
    (n, r, q, o)
}

#[test]
fn free_agreement_requires_both_signatures_exact_terms_and_confirmation() {
    let (mut n, _, q, o) = through_order();
    assert!(n.confirmed().is_none());
    let a = ack(&o, &q, true);
    assert_eq!(
        n.ingest(a.clone(), None, &Profile, 150, 0).unwrap(),
        Ingest::Applied
    );
    let exact = n.confirmed().unwrap().clone();
    n.check_order_ref(&exact).unwrap();
    assert_eq!(exact.order_id, "cc".repeat(32));
    assert_eq!(n.retained().count(), 4);
    assert_eq!(
        n.ingest(a, None, &Profile, 5000, 0).unwrap(),
        Ingest::Duplicate
    );
    let mut changed = exact.clone();
    changed.order_id = "dd".repeat(32);
    assert_eq!(
        n.check_order_ref(&changed).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    changed = exact;
    changed.confirmation.size += 1;
    assert!(n.check_order_ref(&changed).is_err());
}

#[test]
fn closed_terms_reject_semantic_drift_and_keep_meta_inert() {
    let valid = terms();
    assert!(parse_terms(&jcs(&valid).unwrap()).is_ok());
    for (field, value) in [
        ("v", json!("openagents.market-terms.v9")),
        ("requires", json!(["spend"])),
        ("hidden_grant", json!(true)),
        ("meta", json!("active instruction")),
        ("price_msat", json!(1)),
        ("fee_limit_msat", json!(1)),
        ("network", json!("bitcoin")),
        ("order_confirm_by", json!(300)),
        ("review_due_at", json!(300)),
        ("buyer", json!(public(2).to_string())),
        ("price_msat", json!(0.5)),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        assert!(
            parse_terms(&jcs(&changed).unwrap()).is_err(),
            "accepted {field}"
        );
    }
    let mut inert = valid.clone();
    inert["meta"] = json!({"instruction":"not executed"});
    assert!(parse_terms(&jcs(&inert).unwrap()).is_ok());
    let mut missing = valid;
    missing.as_object_mut().unwrap().remove("worker");
    assert!(parse_terms(&jcs(&missing).unwrap()).is_err());
    assert!(parse_terms(b"{\"v\":1,\"v\":2}").is_err());
    assert!(parse_terms(serde_json::to_string_pretty(&terms()).unwrap().as_bytes()).is_err());
}

#[test]
fn offering_checks_signature_digest_features_and_bounds() {
    let e = offering_event(&offer_body());
    assert!(parse_offering(&e).is_ok());
    let mut forged = e.clone();
    forged.pubkey = public(1).to_string();
    assert!(parse_offering(&forged).is_err());
    let mut wrong = offer_body();
    wrong["provider"] = json!(public(1).to_string());
    assert!(parse_offering(&offering_event(&wrong)).is_err());
    for (field, value) in [
        ("profiles", json!(["a", "a"])),
        ("payment_profiles", json!(["x402"])),
        ("networks", json!(["bitcoin"])),
        ("valid_until", json!(100)),
        ("summary", json!("x".repeat(2049))),
        ("profiles", json!(["x".repeat(129)])),
    ] {
        let mut changed = offer_body();
        changed[field] = value;
        assert!(
            parse_offering(&offering_event(&changed)).is_err(),
            "accepted {field}"
        );
    }
    let mut tags = e.tags.clone();
    tags[1] = Tag::new(vec!["x".into(), "00".repeat(32)]);
    let wrong = signer(2).sign(100, OFFERING_KIND, tags, e.content);
    assert!(parse_offering(&wrong).is_err());
}

#[test]
fn head_freshness_and_availability_never_mutate_existing_agreement() {
    let o = offering();
    let value = json!({"v":HEAD_SCHEMA,"requires":[],"provider":public(2).to_string(),"offer":"noop","offering":event_value(o.event()),"status":"paused","valid_until":300});
    let event = signer(2).sign(
        100,
        HEAD_KIND,
        vec![
            Tag::new(vec!["t".into(), "oa:market-head:v1".into()]),
            Tag::new(vec!["d".into(), "noop".into()]),
        ],
        String::from_utf8(jcs(&value).unwrap()).unwrap(),
    );
    assert_eq!(check_head(&event, &o, 200).unwrap(), Availability::Paused);
    assert_eq!(
        check_head(&event, &o, 300).unwrap_err().code,
        RefusalCode::Stale
    );
    let mut foreign = value;
    foreign["offering"]["id"] = json!("dd".repeat(32));
    let event = signer(2).sign(
        100,
        HEAD_KIND,
        event.tags,
        String::from_utf8(jcs(&foreign).unwrap()).unwrap(),
    );
    assert!(check_head(&event, &o, 200).is_err());
}

#[test]
fn record_signer_role_and_envelope_schema_are_checked() {
    let r = rfq();
    let value = parse_strict(&r.canonical).unwrap();
    assert_eq!(
        record(&value, 2, 9).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let mut wrong = value.clone();
    wrong["issuer"] = json!(public(2).to_string());
    assert_eq!(
        record(&wrong, 2, 9).unwrap_err().code,
        RefusalCode::NotAdmitted
    );
    let envelope = private(&value, TERMS_SCHEMA, 1, 2, 9);
    assert!(parse_record(&envelope, None).is_err());
    let envelope = private(&value, RECORD_SCHEMA, 1, 3, 9);
    assert_eq!(
        parse_record(&envelope, None).unwrap_err().code,
        RefusalCode::NotAdmitted
    );
    let mut leak = value;
    leak["body"]["spend"] = json!(true);
    assert_eq!(
        record(&leak, 1, 9).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
}

#[test]
fn unknown_and_payment_record_types_are_not_pass_through_json() {
    for kind in [
        "profile",
        "cancel",
        "payment_instruction",
        "payment_attempt",
        "payment_result",
        "future",
    ] {
        let value = record_value(kind, 1, 0, None, json!({"anything":"goes"}));
        assert_eq!(
            record(&value, 1, 9).unwrap_err().code,
            RefusalCode::UnsupportedFeature
        );
    }
}

#[test]
fn gaps_are_retained_and_can_only_apply_after_the_exact_predecessor() {
    let mut n = negotiation(64);
    let r = rfq();
    let q = quote(&r, &terms(), None, &"bb".repeat(32));
    let o = order(&q, &r, &terms(), &"cc".repeat(32));
    assert_eq!(
        n.ingest(o.clone(), None, &Profile, 140, 0).unwrap(),
        Ingest::Gap
    );
    assert_eq!(n.retained().count(), 1);
    assert_eq!(
        n.ingest(r, None, &Profile, 140, 0).unwrap(),
        Ingest::Applied
    );
    assert_eq!(
        n.ingest(o.clone(), None, &Profile, 140, 0)
            .unwrap_err()
            .code,
        RefusalCode::ContentUnavailable
    );
    n.ingest(q, Some(&document(&terms(), 2).unwrap()), &Profile, 140, 0)
        .unwrap();
    assert_eq!(
        n.ingest(o, None, &Profile, 140, 0).unwrap(),
        Ingest::Applied
    );
}

#[test]
fn equivocation_retains_both_bodies_and_freezes_confirmation() {
    let (mut n, r, q, o) = through_order();
    let mut changed = parse_strict(&r.canonical).unwrap();
    changed["body"]["price_limit_msat"] = json!(1);
    let conflicting = record(&changed, 1, 9).unwrap();
    assert_eq!(
        n.ingest(conflicting, None, &Profile, 150, 0).unwrap(),
        Ingest::Conflict
    );
    assert!(n.conflicted());
    assert_eq!(n.retained().count(), 4);
    assert_eq!(
        n.ingest(ack(&o, &q, true), None, &Profile, 150, 0).unwrap(),
        Ingest::Conflict
    );
    assert!(n.confirmed().is_none());
}

#[test]
fn changed_envelopes_preserve_logical_replay_and_exact_provenance() {
    let mut n = negotiation(64);
    let r = rfq();
    n.ingest(r.clone(), None, &Profile, 120, 0).unwrap();
    let copied = record(&parse_strict(&r.canonical).unwrap(), 1, 44).unwrap();
    assert_ne!(copied.declarations, r.declarations);
    assert_eq!(
        n.ingest(copied.clone(), None, &Profile, 5000, 0).unwrap(),
        Ingest::Duplicate
    );
    let mut q = parse_strict(&quote(&r, &terms(), None, &"bb".repeat(32)).canonical).unwrap();
    q["body"]["rfq"]["event"] = event_value(&copied.declarations[0]);
    n.ingest(
        record(&q, 2, 10).unwrap(),
        Some(&document(&terms(), 2).unwrap()),
        &Profile,
        130,
        0,
    )
    .unwrap();
    assert_eq!(n.retained().count(), 2);
}

#[test]
fn receipt_time_prevents_backdated_orders_and_confirmations() {
    let mut n = negotiation(64);
    let r = rfq();
    let q = quote(&r, &terms(), None, &"bb".repeat(32));
    let o = order(&q, &r, &terms(), &"cc".repeat(32));
    n.ingest(r, None, &Profile, 120, 0).unwrap();
    n.ingest(q, Some(&document(&terms(), 2).unwrap()), &Profile, 130, 0)
        .unwrap();
    assert_eq!(
        n.ingest(o, None, &Profile, 200, 0).unwrap_err().code,
        RefusalCode::Stale
    );
    let (mut n, _, q, o) = through_order();
    assert_eq!(
        n.ingest(ack(&o, &q, true), None, &Profile, 220, 0)
            .unwrap_err()
            .code,
        RefusalCode::Stale
    );
    assert!(n.confirmed().is_none());
}

#[test]
fn quote_terms_require_exact_provider_declaration_and_supported_domain() {
    assert_eq!(
        document(&terms(), 1).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let mut n = negotiation(64);
    let r = rfq();
    n.ingest(r.clone(), None, &Profile, 120, 0).unwrap();
    let q = quote(&r, &terms(), None, &"bb".repeat(32));
    assert_eq!(
        n.ingest(q.clone(), None, &Profile, 130, 0)
            .unwrap_err()
            .code,
        RefusalCode::ContentUnavailable
    );
    let mut altered = terms();
    altered["worker"] = json!(public(4).to_string());
    assert_eq!(
        n.ingest(
            q.clone(),
            Some(&document(&altered, 2).unwrap()),
            &Profile,
            130,
            0
        )
        .unwrap_err()
        .code,
        RefusalCode::IdentityMismatch
    );
    n.ingest(q, Some(&document(&terms(), 2).unwrap()), &Profile, 130, 0)
        .unwrap();
}

#[test]
fn request_and_terms_closure_cannot_be_treated_as_opaque_profile_success() {
    let mut n = negotiation(64);
    let mut r = parse_strict(&rfq().canonical).unwrap();
    r["body"]["request"] = reference(&json!({"work":"different"}), "fixture.noop.v1");
    assert_eq!(
        n.ingest(record(&r, 1, 9).unwrap(), None, &Profile, 120, 0)
            .unwrap_err()
            .code,
        RefusalCode::IdentityMismatch
    );
    let mut n = negotiation(64);
    let r = rfq();
    n.ingest(r.clone(), None, &Profile, 120, 0).unwrap();
    let mut t = terms();
    t["profile_terms"] = reference(&json!({"work":"different"}), "fixture.noop.v1");
    let q = quote(&r, &t, None, &"bb".repeat(32));
    assert_eq!(
        n.ingest(q, Some(&document(&t, 2).unwrap()), &Profile, 130, 0)
            .unwrap_err()
            .code,
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_quote_cannot_be_consumed_by_a_second_order() {
    let (mut n, _, q, o) = through_order();
    let other = order(&q, &o, &terms(), &"dd".repeat(32));
    assert_eq!(
        n.ingest(other, None, &Profile, 150, 0).unwrap_err().code,
        RefusalCode::IdempotencyConflict
    );
    assert!(n.confirmed().is_none());
}

#[test]
fn one_negotiation_cannot_confirm_two_different_quotes() {
    let (mut n, r, q, o) = through_order();
    let q2 = quote(&r, &terms(), Some(&q), &"dd".repeat(32));
    n.ingest(
        q2.clone(),
        Some(&document(&terms(), 2).unwrap()),
        &Profile,
        150,
        0,
    )
    .unwrap();
    let o2 = order(&q2, &o, &terms(), &"ee".repeat(32));
    n.ingest(o2.clone(), None, &Profile, 160, 0).unwrap();
    let a = ack(&o, &q2, true);
    n.ingest(a.clone(), None, &Profile, 170, 0).unwrap();
    assert_eq!(
        n.ingest(ack(&o2, &a, true), None, &Profile, 180, 0)
            .unwrap_err()
            .code,
        RefusalCode::NotAdmitted
    );
    assert_eq!(n.confirmed().unwrap().order_id, "cc".repeat(32));
}

#[test]
fn refused_decision_is_not_overwritten_by_a_later_confirmation() {
    let (mut n, _, q, o) = through_order();
    let refusal = ack(&o, &q, false);
    n.ingest(refusal.clone(), None, &Profile, 150, 0).unwrap();
    assert!(n.confirmed().is_none());
    assert_eq!(
        n.ingest(ack(&o, &refusal, true), None, &Profile, 160, 0)
            .unwrap_err()
            .code,
        RefusalCode::IdempotencyConflict
    );
    assert!(n.confirmed().is_none());
}

#[test]
fn paid_terms_parse_but_cannot_reach_the_no_spend_negotiator() {
    let mut t = terms();
    t["price_msat"] = json!(1000);
    t["payment_profile"] = json!(LIGHTNING_PROFILE);
    t["network"] = json!("bitcoin");
    assert_eq!(parse_terms(&jcs(&t).unwrap()).unwrap().price_msat, 1000);
    let mut offer = offer_body();
    offer["payment_profiles"] = json!([FREE_PROFILE, LIGHTNING_PROFILE]);
    offer["networks"] = json!(["bitcoin"]);
    let offer = parse_offering(&offering_event(&offer)).unwrap();
    let mut r = parse_strict(&rfq().canonical).unwrap();
    r["body"]["offering"] = event_value(offer.event());
    r["body"]["price_limit_msat"] = json!(1000);
    let r = record(&r, 1, 9).unwrap();
    let mut n = Negotiation::new(offer, &public(1).to_string(), &"aa".repeat(32), 64).unwrap();
    n.ingest(r.clone(), None, &Profile, 120, 0).unwrap();
    let q = quote(&r, &t, None, &"bb".repeat(32));
    assert_eq!(
        n.ingest(q, Some(&document(&t, 2).unwrap()), &Profile, 130, 0)
            .unwrap_err()
            .code,
        RefusalCode::UnsupportedFeature
    );
}

#[test]
fn bounded_retention_refuses_without_dropping_previous_evidence() {
    let mut n = negotiation(1);
    let r = rfq();
    n.ingest(r.clone(), None, &Profile, 120, 0).unwrap();
    let q = quote(&r, &terms(), None, &"bb".repeat(32));
    assert_eq!(
        n.ingest(q, Some(&document(&terms(), 2).unwrap()), &Profile, 130, 0)
            .unwrap_err()
            .code,
        RefusalCode::LimitExceeded
    );
    assert_eq!(n.retained().count(), 1);
    assert_eq!(
        n.ingest(r, None, &Profile, 130, 0).unwrap(),
        Ingest::Duplicate
    );
}

#[test]
fn external_artifacts_need_exact_bytes_and_cannot_supply_a_forged_author() {
    let value = parse_strict(&rfq().canonical).unwrap();
    let bytes = jcs(&value).unwrap();
    let envelope = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference(&value,RECORD_SCHEMA),"inline":null,"issued_at":100,"retain_until":1000});
    let event = seal(
        &envelope,
        &secret(1),
        &public(2),
        &"aa".repeat(32),
        110,
        [55; 32],
    )
    .unwrap();
    let opened = open(&event, &secret(2)).unwrap();
    assert_eq!(
        parse_record(&opened, None).unwrap_err().code,
        RefusalCode::ContentUnavailable
    );
    assert!(parse_record(&opened, Some(&bytes)).is_ok());
    assert_eq!(
        parse_record(&opened, Some(b"{}")).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
}
