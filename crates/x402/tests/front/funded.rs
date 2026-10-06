//! Fake funding and actual ledger obligations, with durable inbox fixtures.
use super::*;
use openagents_x402::execution::{ExecutionFront, Resource, TaskInbox};
use openagents_x402::replay::{ReplayEntry, ReplayError, ReplayStore};

const NOW: u64 = 1_790_928_000;
struct CrashReplay {
    store: FileReplayStore,
    crash: AtomicBool,
    consumed: AtomicUsize,
}
impl ReplayStore for CrashReplay {
    fn insert(&self, entry: &ReplayEntry) -> Result<(), ReplayError> {
        self.store.insert(entry)?;
        self.consumed.fetch_add(1, Ordering::SeqCst);
        if self.crash.swap(false, Ordering::SeqCst) {
            panic!("injected crash after proof consumption");
        }
        Ok(())
    }
    fn get(&self, key: &str) -> Result<Option<ReplayEntry>, ReplayError> {
        self.store.get(key)
    }
    fn release(&self, key: &str) -> Result<(), ReplayError> {
        self.store.release(key)
    }
    fn sweep(&self, now: u64) -> Result<usize, ReplayError> {
        self.store.sweep(now)
    }
}
struct Book {
    ledger: Mutex<pay_ledger::Ledger>,
    fail: AtomicBool,
    seen: Mutex<Vec<Settlement>>,
}
impl SettlementSink for Book {
    fn on_settled(&self, s: &Settlement) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("ledger temporarily unavailable".into());
        }
        self.ledger
            .lock()
            .unwrap()
            .record_settlement(pay_ledger::SettlementInput {
                key: s.payment_hash.clone(),
                resource: s.resource.clone(),
                plugin_id: s.plugin.clone(),
                release_id: s.release.clone(),
                price_msat: s.price_msat as i64,
                received_msat: s.received_msat as i64,
                rail: pay_ledger::Rail::Lightning,
                payer_alias: None,
                settled_at: s.settled_at as i64,
                split: pay_ledger::Split::Plugin {
                    author: s.author.clone().unwrap(),
                    fee_msat: s.fee_msat.unwrap() as i64,
                },
            })
            .map_err(|e| e.to_string())?;
        self.seen.lock().unwrap().push(s.clone());
        Ok(())
    }
}
struct Gate {
    entered: std::sync::Barrier,
    release: std::sync::Barrier,
}

struct Inbox {
    directory: PathBuf,
    creates: AtomicUsize,
    lose_after_create: AtomicBool,
    allow: AtomicBool,
    seen_releases: Mutex<Vec<String>>,
    gate: Mutex<Option<Arc<Gate>>>,
}
impl TaskInbox for Inbox {
    fn created(&self, execution: &str) -> Result<Option<String>, String> {
        match std::fs::read_to_string(self.directory.join(execution)) {
            Ok(task) => Ok(Some(task)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn create(&self, execution: &str, call: &Call<'_>) -> Result<String, String> {
        assert!(
            call.request.headers.is_empty(),
            "unbound transport headers and proof must not reach the task inbox"
        );
        if !self.allow.load(Ordering::SeqCst) {
            return Err("execution authority missing".into());
        }
        let gate = self.gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.entered.wait();
            gate.release.wait();
        }
        self.creates.fetch_add(1, Ordering::SeqCst);
        self.seen_releases
            .lock()
            .unwrap()
            .push(call.quote.unwrap().release.clone().unwrap());
        let task = hex::encode(Sha256::digest(format!("task:{execution}")));
        let mut file =
            std::fs::File::create_new(self.directory.join(execution)).map_err(|e| e.to_string())?;
        file.write_all(task.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        if self.lose_after_create.swap(false, Ordering::SeqCst) {
            return Err("task acknowledgment lost".into());
        }
        Ok(task)
    }
}
struct Fixture {
    tmp: tempfile::TempDir,
    node: Arc<TestNode>,
    replay: Arc<CrashReplay>,
    book: Arc<Book>,
    inbox: Arc<Inbox>,
    revision: String,
    release: String,
    amount: u64,
}
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let mut ledger = pay_ledger::Ledger::open(tmp.path().join("ledger.sqlite")).unwrap();
        ledger
            .register_payee(pay_ledger::Payee {
                party: "author".into(),
                destination_kind: "spark".into(),
                destination_value: "fake-no-network".into(),
                source: "account".into(),
                verified_at: NOW as i64,
            })
            .unwrap();
        std::fs::create_dir(tmp.path().join("tasks")).unwrap();
        Self {
            node: Arc::new(TestNode::new(Some(NOW))),
            replay: Arc::new(CrashReplay {
                store: FileReplayStore::open(&tmp.path().join("replay")).unwrap(),
                crash: AtomicBool::new(false),
                consumed: AtomicUsize::new(0),
            }),
            book: Arc::new(Book {
                ledger: Mutex::new(ledger),
                fail: AtomicBool::new(false),
                seen: Mutex::new(Vec::new()),
            }),
            inbox: Arc::new(Inbox {
                directory: tmp.path().join("tasks"),
                creates: AtomicUsize::new(0),
                lose_after_create: AtomicBool::new(false),
                allow: AtomicBool::new(true),
                seen_releases: Mutex::new(Vec::new()),
                gate: Mutex::new(None),
            }),
            tmp,
            revision: "task-route-v1".into(),
            release: "release1".into(),
            amount: 5000,
        }
    }
    fn service(&self) -> ExecutionFront<CrashReplay> {
        self.service_with_receiver(self.node.clone())
    }
    fn service_with_receiver(&self, receiver: Arc<dyn Receiver>) -> ExecutionFront<CrashReplay> {
        let release = self.release.clone();
        let amount = self.amount;
        ExecutionFront::new(
            Config {
                base_url: BASE.into(),
                network: nostr::x402::MAINNET,
                realm: "api.example.com".into(),
                challenge_key: vec![7; 32],
                timeout_secs: 300,
            },
            receiver,
            self.replay.clone(),
            self.book.clone(),
            self.inbox.clone(),
            Resource {
                id: "funded-task".into(),
                revision: self.revision.clone(),
                method: "POST".into(),
                path: "/v1/tasks".into(),
                price: Price::Quote(Arc::new(move |_| {
                    Ok(openagents_x402::front::Quote {
                        price_msat: amount,
                        parts: vec![],
                        plugin: Some("demo".into()),
                        release: Some(release.clone()),
                        author: Some("author".into()),
                        fee_msat: Some(1000),
                        resource: Some("task:demo".into()),
                    })
                })),
                role: "plugin_call".into(),
                resource: "task:demo".into(),
                plugin: Some("demo".into()),
                description: "One admitted task".into(),
                recovery_seconds: 86400,
            },
            "authenticated-account".into(),
            self.tmp.path().join("funded"),
            60,
        )
        .unwrap()
    }
    fn challenge(&self, body: &[u8]) -> Response {
        self.service().handle(&post("/v1/tasks", body, vec![]), NOW)
    }
    fn totals(&self) -> pay_ledger::Totals {
        self.book.ledger.lock().unwrap().totals().unwrap()
    }
}
fn body(key: &str, text: &str) -> Vec<u8> {
    let quote = openagents_x402::front::Quote {
        price_msat: 5000,
        parts: vec![],
        plugin: Some("demo".into()),
        release: Some("release1".into()),
        author: Some("author".into()),
        fee_msat: Some(1000),
        resource: Some("task:demo".into()),
    };
    let quote = openagents_x402::execution::quote_digest(&quote);
    json!({"quote_digest":quote,"funding_scope":"authenticated-account","idempotency_key":key,"request":{"text":text,"admission":"already-authorized-fixture"}})
        .to_string()
        .into_bytes()
}
fn error(response: &Response) -> String {
    serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["type"]
        .as_str()
        .unwrap_or_default()
        .into()
}

#[test]
fn unpaid_is_inert_and_cross_scheme_observation_uses_one_invoice_payment_task_and_obligation() {
    let fixture = Fixture::new();
    let body = body("request1", "Fix the greeting");
    let service = fixture.service();
    let challenge = service.handle(&post("/v1/tasks", &body, vec![]), NOW);
    assert_eq!(challenge.status, 402);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.totals().settlements, 0);
    let repeated = service.handle(&post("/v1/tasks", &body, vec![]), NOW + 1);
    assert_eq!(
        header(&challenge, PAYMENT_REQUIRED),
        header(&repeated, PAYMENT_REQUIRED)
    );
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 1);
    let x402 = x402_signature(&fixture.node, &challenge);
    let payment = payment_credential(&fixture.node, &challenge);
    let first = service.handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, x402.clone())]),
        NOW + 2,
    );
    assert_eq!(
        first.status,
        200,
        "{}",
        String::from_utf8_lossy(&first.body)
    );
    let totals = fixture.totals();
    assert_eq!(totals.settlements, 1);
    assert_eq!(totals.received_msat, 5000);
    assert_eq!(
        totals.accrued_msat + totals.paid_msat + totals.reserved_msat,
        totals.received_msat
    );
    // Recovery authenticates the already consumed proof at its original
    // admission time, even after the invoice's new-payment window closes.
    let second = fixture.service().handle(
        &post(
            "/v1/tasks",
            &body,
            vec![(payment_scheme::AUTHORIZATION, payment)],
        ),
        NOW + 600,
    );
    assert_eq!(
        second.status,
        200,
        "{}",
        String::from_utf8_lossy(&second.body)
    );
    assert_eq!(first.body, second.body);
    assert_eq!(fixture.totals(), totals);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    let seen = fixture.book.seen.lock().unwrap();
    assert_eq!(
        seen[0], seen[1],
        "retry must preserve time, release, author, scheme, and received amount"
    );
}

#[test]
fn crash_after_consumption_recovers_only_the_original_funded_task_and_quote() {
    let mut fixture = Fixture::new();
    let body = body("request2", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let signature = x402_signature(&fixture.node, &challenge);
    fixture.replay.crash.store(true, Ordering::SeqCst);
    let service = fixture.service();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service.handle(
            &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, signature)]),
            NOW + 1,
        )
    }));
    assert!(result.is_err());
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.totals().settlements, 0);
    // A catalog refresh cannot relabel what the original invoice bought.
    fixture.release = "release2".into();
    fixture.amount = 9000;
    let payment = payment_credential(&fixture.node, &challenge);
    let recovered = fixture.service().handle(
        &post(
            "/v1/tasks",
            &body,
            vec![(payment_scheme::AUTHORIZATION, payment)],
        ),
        NOW + 601,
    );
    assert_eq!(
        recovered.status,
        200,
        "{}",
        String::from_utf8_lossy(&recovered.body)
    );
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    assert_eq!(
        *fixture.inbox.seen_releases.lock().unwrap(),
        vec!["release1"]
    );
    assert_eq!(fixture.totals().received_msat, 5000);
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 1);
}

#[test]
fn lost_task_ack_reconciles_the_existing_inbox_key_without_creating_again() {
    let fixture = Fixture::new();
    let body = body("request3", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    fixture
        .inbox
        .lose_after_create
        .store(true, Ordering::SeqCst);
    let signature = x402_signature(&fixture.node, &challenge);
    let first = fixture.service().handle(
        &post(
            "/v1/tasks",
            &body,
            vec![(PAYMENT_SIGNATURE, signature.clone())],
        ),
        NOW + 1,
    );
    assert_eq!(first.status, 500);
    assert!(header(&first, PAYMENT_RESPONSE).is_some());
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    let recovered = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, signature)]),
        NOW + 2,
    );
    assert_eq!(
        recovered.status,
        200,
        "{}",
        String::from_utf8_lossy(&recovered.body)
    );
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.totals().settlements, 1);
}

#[test]
fn changed_bytes_resource_and_revision_conflict_before_consumption() {
    let mut fixture = Fixture::new();
    let original = body("request4", "First request");
    let challenge = fixture.challenge(&original);
    let proof = x402_signature(&fixture.node, &challenge);
    let changed = body("request4", "Changed request");
    let response = fixture.service().handle(
        &post(
            "/v1/tasks",
            &changed,
            vec![(PAYMENT_SIGNATURE, proof.clone())],
        ),
        NOW + 1,
    );
    assert_eq!(
        (response.status, error(&response)),
        (409, "idempotency_conflict".into())
    );
    let response = fixture.service().handle(
        &post(
            "/v1/tasks?different=1",
            &original,
            vec![(PAYMENT_SIGNATURE, proof.clone())],
        ),
        NOW + 1,
    );
    assert_eq!(response.status, 409);
    fixture.revision = "task-route-v2".into();
    let response = fixture.service().handle(
        &post("/v1/tasks", &original, vec![(PAYMENT_SIGNATURE, proof)]),
        NOW + 1,
    );
    assert_eq!(response.status, 409);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 1);
}

#[test]
fn unavailable_ledger_keeps_the_claim_and_restores_one_original_author_obligation() {
    let fixture = Fixture::new();
    let body = body("request5", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let signature = x402_signature(&fixture.node, &challenge);
    fixture.book.fail.store(true, Ordering::SeqCst);
    let first = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, signature)]),
        NOW + 1,
    );
    assert_eq!(first.status, 503);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
    fixture.book.fail.store(false, Ordering::SeqCst);
    let payment = payment_credential(&fixture.node, &challenge);
    let recovered = fixture.service().handle(
        &post(
            "/v1/tasks",
            &body,
            vec![(payment_scheme::AUTHORIZATION, payment)],
        ),
        NOW + 700,
    );
    assert_eq!(
        recovered.status,
        200,
        "{}",
        String::from_utf8_lossy(&recovered.body)
    );
    assert_eq!(fixture.totals().settlements, 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    let book = fixture.book.ledger.lock().unwrap();
    let author = book.available_shares("author").unwrap();
    assert!(
        author
            .iter()
            .any(|share| share.role == "author" && share.amount_msat == 1000)
    );
}

#[test]
fn funded_payment_does_not_supply_missing_execution_authority_and_expired_unpaid_quote_cannot_pay()
{
    let fixture = Fixture::new();
    let body = body("request6", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let expired = fixture
        .service()
        .handle(&post("/v1/tasks", &body, vec![]), NOW + 301);
    assert_eq!(
        (expired.status, error(&expired)),
        (410, "quote_expired".into())
    );
    assert!(header(&expired, PAYMENT_REQUIRED).is_none());
    fixture.inbox.allow.store(false, Ordering::SeqCst);
    let signature = x402_signature(&fixture.node, &challenge);
    let refused = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, signature)]),
        NOW + 1,
    );
    assert_eq!(refused.status, 500);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.totals().settlements, 1);
}

#[test]
fn tampered_proof_cannot_consume_and_payment_first_then_x402_returns_the_same_task() {
    let fixture = Fixture::new();
    let body = body("request7", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let original = x402_signature(&fixture.node, &challenge);
    let mut payload = openagents_x402::wire::decode_payment_payload(&original).unwrap();
    payload
        .payload
        .insert("preimage".into(), json!("0".repeat(64)));
    let bad = openagents_x402::wire::encode_header(&payload).unwrap();
    let refused = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, bad)]),
        NOW + 1,
    );
    assert_eq!(refused.status, 402);
    assert_eq!(
        header(&refused, PAYMENT_REQUIRED),
        header(&challenge, PAYMENT_REQUIRED)
    );
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 0);
    let mut changed: Value = serde_json::from_slice(&body).unwrap();
    changed["funding_scope"] = json!("another-account");
    let wrong_scope = fixture.service().handle(
        &post(
            "/v1/tasks",
            changed.to_string().as_bytes(),
            vec![(PAYMENT_SIGNATURE, original.clone())],
        ),
        NOW + 1,
    );
    assert_eq!(
        (wrong_scope.status, error(&wrong_scope)),
        (403, "funding_scope_mismatch".into())
    );
    let payment = payment_credential(&fixture.node, &challenge);
    let first = fixture.service().handle(
        &post(
            "/v1/tasks",
            &body,
            vec![(payment_scheme::AUTHORIZATION, payment)],
        ),
        NOW + 2,
    );
    assert_eq!(
        first.status,
        200,
        "{}",
        String::from_utf8_lossy(&first.body)
    );
    let followed = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, original)]),
        NOW + 800,
    );
    assert_eq!(
        followed.status,
        200,
        "{}",
        String::from_utf8_lossy(&followed.body)
    );
    assert_eq!(first.body, followed.body);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.totals().settlements, 1);
}

#[test]
fn concurrent_retry_is_busy_until_the_original_host_creation_is_retained() {
    let fixture = Fixture::new();
    let body = body("request8", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let proof = x402_signature(&fixture.node, &challenge);
    let gate = Arc::new(Gate {
        entered: std::sync::Barrier::new(2),
        release: std::sync::Barrier::new(2),
    });
    *fixture.inbox.gate.lock().unwrap() = Some(gate.clone());
    let service = fixture.service();
    let first_request = post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, proof.clone())]);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| service.handle(&first_request, NOW + 1));
        gate.entered.wait();
        let busy = fixture.service().handle(
            &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, proof.clone())]),
            NOW + 2,
        );
        assert_eq!(
            (busy.status, error(&busy)),
            (503, "funded_request_busy".into())
        );
        gate.release.wait();
        let completed = first.join().unwrap();
        assert_eq!(
            completed.status,
            200,
            "{}",
            String::from_utf8_lossy(&completed.body)
        );
        let followed = fixture.service().handle(
            &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, proof)]),
            NOW + 3,
        );
        assert_eq!(completed.body, followed.body);
    });
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.totals().settlements, 1);
}

#[test]
fn corrupt_funding_record_fails_closed_without_issuing_or_consuming_again() {
    let fixture = Fixture::new();
    let body = body("request9", "Fix the greeting");
    let challenge = fixture.challenge(&body);
    let record = std::fs::read_dir(fixture.tmp.path().join("funded"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))
        .unwrap();
    std::fs::write(record, b"{partial").unwrap();
    let proof = x402_signature(&fixture.node, &challenge);
    let response = fixture.service().handle(
        &post("/v1/tasks", &body, vec![(PAYMENT_SIGNATURE, proof)]),
        NOW + 1,
    );
    assert_eq!(
        (response.status, error(&response)),
        (503, "journal_unavailable".into())
    );
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
}

struct WrongAmount {
    node: Arc<TestNode>,
}
impl Receiver for WrongAmount {
    fn pay_to(&self) -> String {
        self.node.pay_to()
    }
    fn invoice(&self, amount: u64, hash: [u8; 32], expiry: u32) -> Result<String, String> {
        self.node.invoice(amount + 1000, hash, expiry)
    }
}
#[test]
fn a_receiver_cannot_offer_the_wrong_amount_before_a_buyer_pays() {
    let fixture = Fixture::new();
    let body = body("request10", "Fix the greeting");
    let service = fixture.service_with_receiver(Arc::new(WrongAmount {
        node: fixture.node.clone(),
    }));
    let response = service.handle(&post("/v1/tasks", &body, vec![]), NOW);
    assert_eq!(
        (response.status, error(&response)),
        (503, "invalid_funding_invoice".into())
    );
    assert!(header(&response, PAYMENT_REQUIRED).is_none());
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
}

#[test]
fn a_changed_quote_identity_is_refused_before_invoice_or_execution() {
    let fixture = Fixture::new();
    let mut request: Value =
        serde_json::from_slice(&body("request11", "Fix the greeting")).unwrap();
    request["quote_digest"] = json!("a".repeat(64));
    let response = fixture.service().handle(
        &post("/v1/tasks", request.to_string().as_bytes(), vec![]),
        NOW,
    );
    assert_eq!(
        (response.status, error(&response)),
        (409, "quote_conflict".into())
    );
    assert_eq!(fixture.node.counter.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.replay.consumed.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.inbox.creates.load(Ordering::SeqCst), 0);
}
