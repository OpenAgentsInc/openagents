use super::*;
use crate::admission::Admission;
use crate::book::{Book, Setup};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::market_contracts::{self as mkt, labor};
use nostr::private_artifact;
use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey};

struct Fixture {
    buyer: SecretKey,
    provider: SecretKey,
    setup: Setup,
    events: Vec<Event>,
    blobs: Blobs,
    now: u64,
}
fn key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn public(key: &SecretKey) -> String {
    key.x_only_public_key(&Secp256k1::new()).0.to_string()
}
fn sign(key: &SecretKey) -> RelaySigner {
    RelaySigner::from_secret_hex(&key.display_secret().to_string()).unwrap()
}
fn sealed(v: &Value, schema: &str, from: &SecretKey, to: &SecretKey, now: u64) -> Event {
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference(v,schema).unwrap(),"inline":v,"issued_at":now,"retain_until":now+2000});
    let nonce = secp256k1::rand::random::<[u8; 32]>();
    let mailbox = nonce.iter().map(|n| format!("{n:02x}")).collect::<String>();
    private_artifact::seal(
        &body,
        from,
        &public(to).parse::<XOnlyPublicKey>().unwrap(),
        &mailbox,
        now,
        nonce,
    )
    .unwrap()
}
fn put(blobs: &mut Blobs, value: Value, schema: &str) -> Value {
    blobs.insert(value, schema).unwrap()
}
fn lock(blobs: &mut Blobs, definition: &Value) -> Value {
    put(
        blobs,
        json!({"v":"openagents.lock.v1","requires":[],"root":definition,"entries":[{"id":definition["id"],"definition":definition,"dependencies":[]}]}),
        "openagents.lock.v1",
    )
}
fn fixture() -> Fixture {
    fixture_with(None)
}
fn fixture_with(custom: Option<(Value, Value)>) -> Fixture {
    let buyer = key(1);
    let provider = key(2);
    let resolver = key(3);
    let now = if custom.is_some() {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    } else {
        100
    };
    let b = public(&buyer);
    let p = public(&provider);
    let r = public(&resolver);
    let mut blobs = Blobs::default();
    let schema = put(
        &mut blobs,
        json!({"type":"object"}),
        "https://json-schema.org/draft/2020-12/schema",
    );
    let mut schema = schema;
    schema["media_type"] = json!("application/schema+json");
    let target_body = json!({"v":1,"requires":[],"id":format!("{p}:labor/command"),"profile":"native","summary":"Admitted local bounded command","input":schema,"output":schema,"effects":{"reads":["workspace"],"writes":["workspace"],"network":[],"process":true,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{},"cancellation":"host_terminated","idempotency":"idempotent","evidence":[]},"binding_contract":{"operation":"coder.free-labor.command.v1","interface":"coder.free-labor.command.v1"}});
    let target_ref = put(&mut blobs, target_body, "openagents.cap.v1");
    let target = json!({"id":format!("{p}:labor/command"),"artifact":target_ref});
    let mut checker_body = blobs.get(&target_ref).unwrap().clone();
    checker_body["id"] = json!(format!("{b}:labor/check"));
    checker_body["effects"]["writes"] = json!([]);
    checker_body["binding_contract"] = json!({"operation":"coder.free-labor.byte-equality.v1","interface":"coder.free-labor.byte-equality.v1"});
    let checker_ref = put(&mut blobs, checker_body, "openagents.cap.v1");
    let checker = json!({"id":format!("{b}:labor/check"),"artifact":checker_ref});
    let target_lock = lock(&mut blobs, &target);
    let checker_lock = lock(&mut blobs, &checker);
    let inert = put(
        &mut blobs,
        json!({"v":"fixture.text.v1","text":"Synthetic fixture. No private source or credentials."}),
        "fixture.text.v1",
    );
    let input = put(
        &mut blobs,
        custom.as_ref().map(|(input,_)|input.clone()).unwrap_or_else(||json!({"v":"coder.free-labor.command.v1","source_revision":"1".repeat(40),"source_snapshot":"sha256:".to_owned()+&"2".repeat(64)})),
        "coder.free-labor.command.v1",
    );
    let disclosure = put(
        &mut blobs,
        json!({"v":"coder.free-labor.disclosure.v1","requires":[],"buyer":b,"worker":p,"mode":"trusted-explicit-command"}),
        "coder.free-labor.disclosure.v1",
    );
    let context = put(
        &mut blobs,
        json!({"v":"openagents.context.v1","requires":[],"task":"f".repeat(64),"recipient":p,"policy":disclosure,"entries":[],"omissions":[],"coverage":"complete"}),
        "openagents.context.v1",
    );
    let requirements = put(
        &mut blobs,
        custom.as_ref().map(|(_,requirements)|requirements.clone()).unwrap_or_else(||json!({"v":"coder.free-labor.requirements.v1","process":true,"network":false,"wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456})),
        "coder.free-labor.requirements.v1",
    );
    let source_input = blobs.get(&input).unwrap().clone();
    let source = put(
        &mut blobs,
        json!({"v":"coder.free-labor.source.v1","requires":[],"input":input,"revision":source_input["intent"]["workspace"]["source_revision"].as_str().unwrap_or(&"1".repeat(40)),"snapshot":source_input["source_snapshot"]}),
        "coder.free-labor.source.v1",
    );
    let snapshot = put(
        &mut blobs,
        json!({"v":"openagents.snapshot.v1","requires":[],"scope":"labor-workspace","captured_at":now,"sources":[{"id":"workspace","version":source,"kind":"repository","availability":"retained"}],"coverage":"complete"}),
        "openagents.snapshot.v1",
    );
    let precedence = put(
        &mut blobs,
        json!({"v":"coder.free-labor.empty-instructions.v1","requires":[],"rule":"explicit-command-only"}),
        "coder.free-labor.empty-instructions.v1",
    );
    let mut resolver_body = blobs.get(&checker_ref).unwrap().clone();
    resolver_body["id"] = json!(format!("{b}:labor/instructions"));
    resolver_body["binding_contract"] = json!({"operation":"coder.free-labor.empty-instructions.v1","interface":"coder.free-labor.empty-instructions.v1"});
    let resolver_artifact = put(&mut blobs, resolver_body, "openagents.cap.v1");
    let resolver = json!({"id":format!("{b}:labor/instructions"),"artifact":resolver_artifact});
    let instructions = put(
        &mut blobs,
        json!({"v":"openagents.instructions.v1","requires":[],"task":"f".repeat(64),"revision":0,"resolver":resolver,"precedence":precedence,"snapshot":snapshot,"entries":[]}),
        "openagents.instructions.v1",
    );
    let frame = put(
        &mut blobs,
        json!({"v":"openagents.task-frame.v1","requires":[],"task":"f".repeat(64),"owner":b,"controller":b,"revision":0,"previous":null,"objective":inert,"origin":"user","constraints":[inert],"acceptance":[inert],"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]}),
        "openagents.task-frame.v1",
    );
    let rights = put(
        &mut blobs,
        json!({"v":labor::RIGHTS_SCHEMA,"requires":[],"license":inert,"input_use":"perform-and-review-order","output_use":"review-only","publication":"deny","training":"deny","evaluation_reuse":"deny","redistribution":"deny","recipients":[b,p,r],"retention":"through-market-retain-until"}),
        labor::RIGHTS_SCHEMA,
    );
    let policy = put(
        &mut blobs,
        json!({"v":labor::ACCEPTANCE_POLICY_SCHEMA,"requires":[],"checker":checker,"lock":checker_lock,"criteria":["result"],"rule":"all-pass-v1"}),
        labor::ACCEPTANCE_POLICY_SCHEMA,
    );
    let terms = put(
        &mut blobs,
        json!({"v":labor::LABOR_TERMS_SCHEMA,"requires":[],"task_frame":frame,"execution":{"target":target,"lock":target_lock,"input":input,"context":context,"requirements":requirements,"bounds":[]},"deliverables":[{"id":"patch","schema":schema,"max_bytes":4096}],"reviewer":b,"acceptance_policy":policy,"resolver":r,"resolver_policy":"labor-evidence-v1","max_reworks":0,"rework_due_at":null,"dispute_due_at":now+450,"resolution_due_at":now+500,"cancellation":"evaluate-delivered-work-v1","partial_delivery":"no-partial-payment-v1","buyer_unavailable":"resolver-required-v1","rights":rights,"role_relationships":[{"pubkey":b,"operator":"single-fixture-operator"},{"pubkey":p,"operator":"single-fixture-operator"},{"pubkey":r,"operator":"single-fixture-operator"}]}),
        labor::LABOR_TERMS_SCHEMA,
    );
    let market = json!({"v":mkt::TERMS_SCHEMA,"requires":[],"profile":labor::PROFILE,"profile_terms":terms,"buyer":b,"provider":p,"worker":p,"price_msat":0,"fee_limit_msat":0,"payment_profile":mkt::FREE_PROFILE,"network":null,"quote_expires_at":now+100,"order_confirm_by":now+120,"delivery_due_at":now+300,"review_due_at":now+400,"payment_due_at":now+600,"retain_until":now+900});
    let terms_event = sealed(&market, mkt::TERMS_SCHEMA, &provider, &buyer, now);
    let offering_body = json!({"v":mkt::OFFERING_SCHEMA,"requires":[],"provider":p,"offer":"fixture","capability":target,"profiles":[labor::PROFILE],"payment_profiles":[mkt::FREE_PROFILE],"networks":[],"summary":"Synthetic free labor fixture","price_hint_msat":0,"capacity_hint":1,"valid_until":now+100});
    let content = String::from_utf8(jcs(&offering_body).unwrap()).unwrap();
    let digest = nostr::contracts::digest_bytes(content.as_bytes());
    let offering = sign(&provider).sign(
        now,
        mkt::OFFERING_KIND,
        vec![
            Tag::new(vec!["t".into(), "oa:market-offering:v1".into()]),
            Tag::new(vec!["x".into(), digest[7..].into()]),
        ],
        content,
    );
    let market_id = "a".repeat(64);
    let rec = |kind: &str, issuer: &str, seq: u64, prev: Value, body: Value| json!({"v":mkt::RECORD_SCHEMA,"requires":[],"type":kind,"market":market_id,"buyer":b,"provider":p,"issuer":issuer,"seq":seq,"prev":prev,"issued_at":now,"body":body});
    let rfq = rec(
        "rfq",
        &b,
        0,
        Value::Null,
        json!({"offering":{"id":offering.id,"pubkey":p,"kind":mkt::OFFERING_KIND},"profile":labor::PROFILE,"request":terms,"price_limit_msat":0,"response_due_at":now+90,"retain_until":now+900}),
    );
    let rfq_ref = reference(&rfq, mkt::RECORD_SCHEMA).unwrap();
    let quote = rec(
        "quote",
        &p,
        0,
        Value::Null,
        json!({"rfq":rfq_ref,"quote_id":"b".repeat(64),"terms":reference(&market,mkt::TERMS_SCHEMA).unwrap()}),
    );
    let quote_ref = reference(&quote, mkt::RECORD_SCHEMA).unwrap();
    let order = rec(
        "order",
        &b,
        1,
        rfq_ref,
        json!({"quote":quote_ref,"terms_digest":reference(&market,mkt::TERMS_SCHEMA).unwrap()["digest"],"order_id":"c".repeat(64)}),
    );
    let ack = rec(
        "order_ack",
        &p,
        1,
        quote_ref,
        json!({"order":reference(&order,mkt::RECORD_SCHEMA).unwrap(),"decision":"confirmed","code":null}),
    );
    let events = vec![
        sealed(&rfq, mkt::RECORD_SCHEMA, &buyer, &provider, now),
        sealed(&quote, mkt::RECORD_SCHEMA, &provider, &buyer, now),
        sealed(&order, mkt::RECORD_SCHEMA, &buyer, &provider, now),
        sealed(&ack, mkt::RECORD_SCHEMA, &provider, &buyer, now),
    ];
    let admission = Admission {
        target,
        checker,
        task_frame: frame,
        input,
        context,
        requirements,
        rights,
        blobs: blobs.clone(),
    };
    Fixture {
        buyer,
        provider,
        setup: Setup {
            market: market_id,
            offering,
            terms: terms_event,
            admission,
        },
        events,
        blobs,
        now,
    }
}
fn agreed(f: &Fixture, key: SecretKey) -> Book {
    let mut b = Book::new(f.setup.clone(), key).unwrap();
    for e in &f.events {
        assert_eq!(b.receive(e, f.now, &Blobs::default()).unwrap(), "applied");
    }
    b
}

#[test]
fn both_roles_confirm_the_exact_order_without_creating_execution() {
    let f = fixture();
    let mut b = Book::new(f.setup.clone(), f.buyer).unwrap();
    for e in &f.events[..3] {
        b.receive(e, f.now, &Blobs::default()).unwrap();
        assert!(b.order().is_none());
    }
    b.receive(&f.events[3], f.now, &Blobs::default()).unwrap();
    let provider = agreed(&f, f.provider);
    assert_eq!(b.order(), provider.order());
    assert!(b.records.link.is_none());
    assert_eq!(
        b.receive(&f.events[3], 99999, &Blobs::default()).unwrap(),
        "duplicate"
    );
}
#[test]
fn durable_store_replays_both_roles_and_retains_duplicates() {
    let f = fixture();
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("buyer");
    {
        let mut s = store::Store::open(&directory, f.setup.clone(), f.buyer).unwrap();
        for e in &f.events {
            s.receive(e.clone(), f.now, Blobs::default()).unwrap();
        }
        assert!(s.book.order().is_some());
        assert!(store::Store::open(&directory, f.setup.clone(), f.buyer).is_err());
    }
    let mut s = store::Store::open(&directory, f.setup.clone(), f.buyer).unwrap();
    assert!(s.book.order().is_some());
    assert_eq!(
        s.receive(f.events[3].clone(), 10000, Blobs::default())
            .unwrap(),
        "duplicate"
    );
    assert_eq!(s.observations().len(), 5);
}
#[test]
fn missing_closure_changed_pin_and_unrelated_reader_refuse() {
    let f = fixture();
    assert!(Book::new(f.setup.clone(), key(4)).is_err());
    let mut setup = f.setup.clone();
    setup
        .admission
        .blobs
        .0
        .remove(setup.admission.input["digest"].as_str().unwrap());
    assert!(Book::new(setup, f.buyer).is_err());
    let mut setup = f.setup;
    setup.admission.target["id"] = json!(format!("{}:labor/other", public(&f.provider)));
    assert!(Book::new(setup, f.buyer).is_err());
}

fn order_value(book: &Book) -> Value {
    let o = book.order().unwrap();
    json!({"market":o.market,"order_id":o.order_id,"buyer":o.buyer,"provider":o.provider,"order":artifact_value(&o.order),"confirmation":artifact_value(&o.confirmation)})
}
fn link(f: &mut Fixture, book: &mut Book) -> Event {
    let execution = &book.labor().execution;
    let body = json!({"v":nostr::execution::SCHEMA,"requires":["openagents.labor-binding.v1"],"type":"execute","request":"labor-request","attempt":1,"run":"labor-run","target":f.setup.admission.target,"lock":artifact_value(&execution.lock),"input":{"artifact":artifact_value(&execution.input)},"context":artifact_value(&execution.context),"requirements":artifact_value(&execution.requirements),"bounds":{"wall_ms":10000,"output_bytes":3145728,"jobs":1},"deadline":book.market().delivery_due_at,"retain_until":book.market().retain_until});
    let encrypted = nostr::execution::Seal {
        signer: &sign(&f.buyer),
        conversation: nostr::nip44::conversation_key(
            &f.buyer,
            &public(&f.provider).parse().unwrap(),
        ),
        nonce: secp256k1::rand::random(),
        created_at: f.now,
    }
    .event(
        nostr::execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), public(&f.provider)]),
            Tag::new(vec![
                "expiration".into(),
                book.market().delivery_due_at.to_string(),
            ]),
        ],
        &body,
    )
    .unwrap();
    let body_ref = put(&mut f.blobs, body, nostr::execution::SCHEMA);
    let linkage = json!({"v":records::LINK,"requires":[],"issuer":public(&f.buyer),"order":order_value(book),"request":"labor-request","attempt":1,"run":"labor-run","execute":{"id":encrypted.id,"pubkey":encrypted.pubkey,"kind":encrypted.kind},"execute_body":body_ref,"rework":null,"context":f.setup.admission.context,"previous":null});
    let event = sealed(&linkage, records::LINK, &f.buyer, &f.provider, f.now);
    assert_eq!(book.receive(&event, f.now, &f.blobs).unwrap(), "applied");
    encrypted
}
fn run_evidence(
    f: &mut Fixture,
    book: &Book,
    output: &Value,
    observation: &Value,
    outcome: &str,
) -> Vec<Value> {
    let execution = &book.labor().execution;
    let data = vec![
        (
            "created",
            json!({"owner":public(&f.provider),"request":"labor-request","base":f.setup.admission.input,"program":f.setup.admission.target,"lock":artifact_value(&execution.lock),"context":f.setup.admission.context,"policy":f.setup.admission.requirements,"parent":null,"recipients":[public(&f.buyer),public(&f.provider)]}),
        ),
        (
            "admitted",
            json!({"enforcement":f.setup.admission.requirements,"reservations":[],"deadlines":{"delivery":book.market().delivery_due_at},"retention":book.market().retain_until,"input":f.setup.admission.input}),
        ),
        (
            "dispatched",
            json!({"binding":f.setup.admission.target,"attempt":1,"input":f.setup.admission.input,"context":f.setup.admission.context,"effect":"fixture-effect","generation":0}),
        ),
        (
            "resolved",
            json!({"outcome":outcome,"dispatched":true,"output":output,"receipts":[observation],"usage":{"cost_usd":null,"reason":"fixture_only"},"verification":"not_run","integration":"unconfirmed"}),
        ),
    ];
    let mut previous = Value::Null;
    let mut refs = vec![];
    for (seq, (kind, data)) in data.into_iter().enumerate() {
        let record = json!({"run":"labor-run","seq":seq,"previous":previous,"controller":public(&f.provider),"generation":0,"type":kind,"subject":{"step":"command","iteration":0,"attempt":1},"time":f.now,"data":data});
        let digest = nostr::run::logical_digest(&record).unwrap();
        let envelope =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        refs.push(put(&mut f.blobs, envelope, nostr::run::RECORD_SCHEMA));
        previous = json!(digest);
    }
    refs
}
fn deliver(f: &mut Fixture, book: &mut Book, received_at: u64, verdict: &str) {
    link(f, book);
    let output = put(
        &mut f.blobs,
        json!({"v":"fixture.patch.v1","patch":"synthetic patch bytes"}),
        "fixture.patch.v1",
    );
    let observation = put(
        &mut f.blobs,
        json!({"v":"fixture.observation.v1","statement":"Fixture observation only; no real execution claim."}),
        "fixture.observation.v1",
    );
    let runs = run_evidence(f, book, &output, &observation, "completed");
    let submission = json!({"v":records::SUBMISSION,"requires":[],"issuer":public(&f.provider),"order":order_value(book),"number":0,"previous":null,"rework":null,"executions":[book.records.link],"deliverables":[{"id":"patch","content":output}],"run_evidence":runs,"limitations":observation});
    let event = sealed(
        &submission,
        records::SUBMISSION,
        &f.provider,
        &f.buyer,
        f.now,
    );
    assert_eq!(book.receive(&event, f.now, &f.blobs).unwrap(), "applied");
    let delivery = json!({"v":records::DELIVERY,"requires":[],"issuer":public(&f.buyer),"order":order_value(book),"submission":book.records.submission,"received_at":received_at,"available":true});
    let event = sealed(&delivery, records::DELIVERY, &f.buyer, &f.provider, f.now);
    assert_eq!(
        book.receive(&event, received_at, &f.blobs).unwrap(),
        "applied"
    );
    let criteria = json!([{"id":"result","verdict":verdict,"evidence":[observation]}]);
    let checker_receipt = put(
        &mut f.blobs,
        json!({"v":"openagents.free-labor.checker.v1","requires":[],"submission":book.records.submission,"checker":f.setup.admission.checker,"lock":artifact_value(&book.policy().lock),"input":f.setup.admission.input,"criteria":criteria,"verdict":verdict,"elapsed_ms":0,"cost_usd":null,"evidence":[observation],"limitations":observation}),
        "openagents.free-labor.checker.v1",
    );
    let verification = json!({"v":records::VERIFICATION,"requires":[],"issuer":public(&f.buyer),"order":order_value(book),"submission":book.records.submission,"policy":artifact_value(&book.labor().acceptance_policy),"checker_receipts":[checker_receipt],"criteria":[{"id":"result","verdict":verdict,"evidence":[observation]}],"verdict":verdict,"limitations":observation});
    let event = sealed(
        &verification,
        records::VERIFICATION,
        &f.buyer,
        &f.provider,
        f.now,
    );
    assert_eq!(
        book.receive(&event, received_at, &f.blobs).unwrap(),
        "applied"
    );
}
fn review(f: &Fixture, book: &Book, decision: &str) -> Event {
    let reason = f
        .blobs
        .0
        .iter()
        .find(|(_, v)| v["v"] == "fixture.observation.v1")
        .map(|(_, v)| reference(v, "fixture.observation.v1").unwrap())
        .unwrap();
    let body = json!({"v":records::REVIEW,"requires":[],"issuer":public(&f.buyer),"order":order_value(book),"submission":book.records.submission,"verification":book.records.verification,"decision":decision,"criteria":if decision=="accept"{json!([])}else{json!(["result"])},"reason":reason});
    sealed(&body, records::REVIEW, &f.buyer, &f.provider, f.now)
}
#[test]
fn worker_requires_durable_linkage_and_generic_workers_reject_labor_feature() {
    let mut f = fixture();
    let mut book = agreed(&f, f.provider);
    let event = link(&mut f, &mut book);
    assert!(book.check_execute(&event, f.now).is_ok());
    assert!(
        nostr::execution::open_request(
            &event,
            &public(&f.provider),
            &f.provider,
            f.now,
            nostr::execution::Window::DEFAULT
        )
        .is_err()
    );
    let without = agreed(&f, f.provider);
    assert!(without.check_execute(&event, f.now).is_err());
    assert!(book.check_execute(&event, 401).is_err());
}
#[test]
fn late_or_failed_or_unknown_checks_cannot_be_buyer_accepted() {
    for (time, verdict) in [
        (401, "passed"),
        (200, "failed"),
        (200, "unverifiable"),
        (200, "not_run"),
    ] {
        let mut f = fixture();
        let mut book = agreed(&f, f.provider);
        deliver(&mut f, &mut book, time, verdict);
        assert!(
            book.receive(&review(&f, &book, "accept"), time, &f.blobs)
                .is_err()
        );
        assert!(book.records.acceptance.is_none());
    }
}
#[test]
fn buyer_acceptance_remains_separate_from_successful_verification_and_payment() {
    let mut f = fixture();
    let mut book = agreed(&f, f.provider);
    deliver(&mut f, &mut book, 200, "passed");
    assert!(book.records.acceptance.is_none());
    assert_eq!(
        book.receive(&review(&f, &book, "accept"), 200, &f.blobs)
            .unwrap(),
        "applied"
    );
    let body = json!({"v":records::ACCEPTANCE,"requires":[],"issuer":public(&f.buyer),"order":order_value(&book),"submission":book.records.submission,"verification":book.records.verification,"outcome":"accepted","basis":"buyer_acceptance","review":book.records.review,"resolution":null,"amount_due_msat":0,"supersedes":[],"evidence":[book.records.delivery]});
    let event = sealed(&body, records::ACCEPTANCE, &f.buyer, &f.provider, f.now);
    assert_eq!(
        book.receive(&event, 200, &Blobs::default()).unwrap(),
        "applied"
    );
    assert!(book.records.acceptance.is_some());
    assert_eq!(book.market().price_msat, 0);
}
#[test]
fn changed_attachment_bytes_and_zero_bound_rework_are_retained_refusals() {
    let mut f = fixture();
    let mut book = agreed(&f, f.provider);
    deliver(&mut f, &mut book, 200, "failed");
    assert!(
        book.receive(&review(&f, &book, "request_rework"), 200, &f.blobs)
            .unwrap_err()
            .contains("zero-rework")
    );
    let mut bad = f.blobs.clone();
    let value = bad.0.values_mut().next().unwrap();
    *value = json!({"substituted":true});
    assert!(
        book.receive(&review(&f, &book, "reject"), 200, &bad)
            .unwrap_err()
            .contains("digest mismatch")
    );
}

mod relay;

#[tokio::test]
async fn encrypted_relay_auth_reconnect_recipient_and_persisted_order() {
    let f = fixture();
    let (url, relay, events) = relay::start().await;
    let directory = tempfile::tempdir().unwrap();
    let buyer_path = directory.path().join("buyer");
    let provider_path = directory.path().join("provider");
    let mut buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    for event in &f.events {
        let secret = if event.pubkey == public(&f.buyer) {
            &f.buyer
        } else {
            &f.provider
        };
        transport::publish(&url, secret, event).await.unwrap();
        transport::publish(&url, secret, event).await.unwrap();
        // Each read opens a new authenticated connection after the publisher
        // disconnects. Local receipts survive beyond either socket lifetime.
        let from_relay = transport::fetch(&url, &f.buyer, &event.id).await.unwrap();
        assert_eq!(
            buyer.receive(from_relay, f.now, Blobs::default()).unwrap(),
            "applied"
        );
        let from_relay = transport::fetch(&url, &f.provider, &event.id)
            .await
            .unwrap();
        assert_eq!(
            provider
                .receive(from_relay.clone(), f.now, Blobs::default())
                .unwrap(),
            "applied"
        );
        assert_eq!(
            provider
                .receive(from_relay, f.now, Blobs::default())
                .unwrap(),
            "duplicate"
        );
    }
    assert!(
        transport::fetch(&url, &key(9), &f.events[0].id)
            .await
            .is_err()
    );
    assert!(
        transport::publish(&url, &key(9), &f.events[0])
            .await
            .is_err()
    );
    let mut bad = f.events[0].clone();
    bad.content.push('x');
    assert!(transport::publish(&url, &f.buyer, &bad).await.is_err());
    assert_eq!(events.lock().await.len(), 4);
    let order = provider.book.order().unwrap().clone();
    drop(buyer);
    drop(provider);
    relay.abort();
    // A process restart reconstructs the order without relying on relay uptime.
    let buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let provider = store::Store::open(&provider_path, f.setup, f.provider).unwrap();
    assert_eq!(buyer.book.order(), Some(&order));
    assert_eq!(provider.book.order(), Some(&order));
}

#[test]
fn missing_journal_or_lock_never_recreates_agreement_history() {
    for name in ["labor.json", "labor.lock"] {
        let f = fixture();
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("store");
        drop(store::Store::open(&directory, f.setup.clone(), f.buyer).unwrap());
        std::fs::remove_file(directory.join(name)).unwrap();
        assert!(store::Store::open(&directory, f.setup, f.buyer).is_err());
        assert!(!directory.join(name).exists());
    }
}
#[test]
fn replaced_lock_refuses_mutation_and_overflow_keeps_original_blobs() {
    let f = fixture();
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("store");
    let mut store = store::Store::open(&directory, f.setup, f.buyer).unwrap();
    std::fs::rename(directory.join("labor.lock"), directory.join("old.lock")).unwrap();
    std::fs::write(directory.join("labor.lock"), []).unwrap();
    assert!(
        store
            .receive(f.events[0].clone(), f.now, Blobs::default())
            .is_err()
    );
    let mut blobs = Blobs::default();
    for n in 0..256 {
        blobs.insert(json!({"n":n}), "fixture.v1").unwrap();
    }
    let before = blobs.0.clone();
    assert!(blobs.insert(json!({"n":999}), "fixture.v1").is_err());
    assert_eq!(blobs.0, before);
}

mod roundtrip;

#[test]
fn unknown_execution_cannot_be_delivered_and_dispute_blocks_acceptance() {
    let mut f = fixture();
    let mut book = agreed(&f, f.provider);
    link(&mut f, &mut book);
    let evidence = put(
        &mut f.blobs,
        json!({"synthetic":true,"result":"unknown"}),
        "fixture.observation.v1",
    );
    let runs = run_evidence(&mut f, &book, &evidence, &evidence, "unknown");
    let body = json!({"v":records::SUBMISSION,"requires":[],"issuer":public(&f.provider),"order":order_value(&book),"number":0,"previous":null,"rework":null,"executions":[book.records.link],"deliverables":[{"id":"patch","content":evidence}],"run_evidence":runs,"limitations":evidence});
    let event = sealed(&body, records::SUBMISSION, &f.provider, &f.buyer, f.now);
    assert!(
        book.receive(&event, f.now, &f.blobs)
            .unwrap_err()
            .contains("completed attempt")
    );
    assert!(book.records.submission.is_none());

    let mut f = fixture();
    let mut book = agreed(&f, f.provider);
    deliver(&mut f, &mut book, 200, "passed");
    book.receive(&review(&f, &book, "accept"), 200, &f.blobs)
        .unwrap();
    let dispute = json!({"v":records::DISPUTE,"requires":[],"issuer":public(&f.provider),"order":order_value(&book),"subject":book.records.review,"cause":"review","evidence":[book.records.delivery]});
    let event = sealed(&dispute, records::DISPUTE, &f.provider, &f.buyer, f.now);
    assert_eq!(book.receive(&event, 200, &f.blobs).unwrap(), "applied");
    assert_eq!(book.records.disputes.len(), 1);
    let acceptance = json!({"v":records::ACCEPTANCE,"requires":[],"issuer":public(&f.buyer),"order":order_value(&book),"submission":book.records.submission,"verification":book.records.verification,"outcome":"accepted","basis":"buyer_acceptance","review":book.records.review,"resolution":null,"amount_due_msat":0,"supersedes":[],"evidence":[book.records.delivery]});
    let event = sealed(
        &acceptance,
        records::ACCEPTANCE,
        &f.buyer,
        &f.provider,
        f.now,
    );
    assert!(
        book.receive(&event, 200, &f.blobs)
            .unwrap_err()
            .contains("resolver reconciliation")
    );
    assert!(book.records.acceptance.is_none());
}
