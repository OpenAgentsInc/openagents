use super::*;
use crate::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentPayload, ReplayStore,
    front::{self, Call, Config, Front, Price, PricePart, Route, SettlementSink},
    server::{Receiver, Request},
    wire,
};
use nostr::x402::{
    MAINNET, binding_hash, http_binding,
    test_invoice::{number, payee_of, signed_by, tag, words},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
const NOW: u64 = 1_792_022_460;
const SECRET: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Wallet {
    counter: AtomicU64,
    preimages: Mutex<BTreeMap<String, [u8; 32]>>,
    received: Mutex<BTreeMap<String, u64>>,
}
impl Receiver for Wallet {
    fn pay_to(&self) -> String {
        hex::encode(payee_of([9; 32]))
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        let mut seed = request_hash.to_vec();
        seed.extend(self.counter.fetch_add(1, Ordering::SeqCst).to_be_bytes());
        let preimage: [u8; 32] = Sha256::digest(seed).into();
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&request_hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(hash), preimage);
        Ok(signed_by(
            [9; 32],
            &format!("lnbc{}n", amount / 100),
            fields,
            false,
            false,
            NOW,
        ))
    }
    fn received_msat(&self, hash: [u8; 32]) -> Result<Option<u64>, String> {
        Ok(self
            .received
            .lock()
            .unwrap()
            .get(&hex::encode(hash))
            .copied())
    }
}
struct Sink {
    ledger: Mutex<pay_ledger::Ledger>,
    fail: AtomicU64,
    pause: Option<std::path::PathBuf>,
}
impl SettlementSink for Sink {
    fn on_settled(&self, s: &Settlement) -> Result<(), String> {
        let input = pay_ledger::SettlementInput {
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
        };
        if self.fail.load(Ordering::SeqCst) == 1 {
            return Err("Synthetic unavailable settlement sink.".into());
        }
        self.ledger
            .lock()
            .unwrap()
            .record_settlement(input)
            .map_err(|e| e.to_string())?;
        if self.fail.load(Ordering::SeqCst) == 2 {
            panic!("Synthetic crash after durable ledger append.");
        }
        if self.fail.load(Ordering::SeqCst) == 3 {
            let file = File::create(self.pause.as_ref().unwrap()).unwrap();
            file.sync_all().unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Ok(())
    }
}
struct Root {
    path: std::path::PathBuf,
    _owner: Option<tempfile::TempDir>,
}
impl Root {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
struct Fixture {
    root: Root,
    wallet: Arc<Wallet>,
    sink: Arc<Sink>,
    count: Arc<AtomicU64>,
    quote: Quote,
    body: Vec<u8>,
    hash: String,
    payment: PaymentPayload,
}
impl Fixture {
    fn new() -> Self {
        let owner = tempfile::tempdir().unwrap();
        let root = Root {
            path: owner.path().to_path_buf(),
            _owner: Some(owner),
        };
        let wallet = Arc::new(Wallet {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(BTreeMap::new()),
            received: Mutex::new(BTreeMap::new()),
        });
        let sink = Arc::new(Sink {
            ledger: Mutex::new(
                pay_ledger::Ledger::open(&root.path().join("ledger.sqlite")).unwrap(),
            ),
            fail: AtomicU64::new(0),
            pause: None,
        });
        let count = Arc::new(AtomicU64::new(0));
        let quote = Quote {
            price_msat: 6000,
            parts: vec![
                PricePart {
                    name: "endpoint".into(),
                    msat: 5000,
                },
                PricePart {
                    name: "author_fee".into(),
                    msat: 1000,
                },
            ],
            plugin: Some("a:notes".into()),
            release: Some("b".repeat(64)),
            author: Some("c".repeat(64)),
            fee_msat: Some(1000),
            resource: None,
        };
        let body=serde_json::to_vec(&json!({"request":"Private supplied notes","quote_digest":crate::execution::quote_digest(&quote),"recovery_authorization":commitment(SECRET)})).unwrap();
        let mut f=Self {root,wallet,sink,count,quote,body,hash:String::new(),payment:PaymentPayload {x402_version:2,resource:None,accepted:serde_json::from_value(json!({"scheme":"exact","network":MAINNET,"amount":"6000","asset":"BTC","payTo":hex::encode(payee_of([9;32])),"maxTimeoutSeconds":300,"extra":{}})).unwrap(),payload:Default::default(),extensions:None}};
        let front = f.front(0);
        let (response, _) = front.handle(&f.request(vec![]), NOW);
        assert_eq!(response.status, 402);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["recovery_contract"],
            SCHEMA
        );
        let required = wire::decode_payment_required(
            response
                .headers
                .iter()
                .find(|(n, _)| n == PAYMENT_REQUIRED)
                .unwrap()
                .1
                .as_str(),
        )
        .unwrap();
        f.payment.accepted = required.accepts[0].clone();
        let invoice =
            nostr::x402::decode_invoice(f.payment.accepted.extra["invoice"].as_str().unwrap())
                .unwrap();
        f.hash = hex::encode(invoice.payment_hash());
        f.payment.payload.insert(
            "preimage".into(),
            json!(hex::encode(f.wallet.preimages.lock().unwrap()[&f.hash])),
        );
        f.wallet
            .received
            .lock()
            .unwrap()
            .insert(f.hash.clone(), 6000);
        f
    }
    fn front(&self, mode: u64) -> Front<FileReplayStore> {
        let price = self.quote.clone();
        let count = self.count.clone();
        let root = self.root.path().to_path_buf();
        Front::new(Config {base_url:"https://fixture.invalid".into(),network:MAINNET,realm:"fixture".into(),challenge_key:vec![7;32],timeout_secs:300},self.wallet.clone(),Facilitator::new(FileReplayStore::open(&self.root.path().join("replay")).unwrap(),60),self.sink.clone(),vec![Route {id:"invoke".into(),method:"POST".into(),path:"/v1/plugins/{id}/invoke".into(),price:Price::Quote(Arc::new(move |_|Ok(price.clone()))),executor:Arc::new(move |_:&Call<'_>| {count.fetch_add(1,Ordering::SeqCst); if mode==1 {return Err("Synthetic known guest failure.".into());} if mode==2 {panic!("Synthetic interrupted guest.");}if mode==4 {let mut f=std::fs::OpenOptions::new().create_new(true).write(true).open(root.join("invoked")).unwrap(); use std::io::Write; f.write_all(b"one admitted invocation").unwrap(); f.sync_all().unwrap(); File::create(root.join("ready")).unwrap().sync_all().unwrap(); loop {std::thread::sleep(std::time::Duration::from_secs(1));}} if mode==3 {for e in std::fs::read_dir(root.join("outcomes")).unwrap(){ let p=e.unwrap().path(); if p.extension().is_some_and(|s|s=="json"){ std::fs::create_dir(p.with_extension("pending")).unwrap();}}} Ok(front::Output {body:serde_json::to_vec(&json!({"private_result":"Only the original purchase secret can retrieve this"})).unwrap(),content_type:Some("application/json".into())})}),role:"plugin_call".into(),resource:"fixture".into(),plugin:None,description:"Synthetic private notes".into(),mime_type:"application/json".into(),model_cost_only:false}]).unwrap().with_outcomes(Store::open(&self.root.path().join("outcomes")).unwrap())
    }
    fn request(&self, headers: Vec<(String, String)>) -> Request {
        Request {
            method: "POST".into(),
            target: "/v1/plugins/notes/invoke".into(),
            headers,
            body: self.body.clone(),
        }
    }
    fn invoke(&self, front: &Front<FileReplayStore>) -> Response {
        front
            .handle(
                &self.request(vec![
                    (AUTHORIZATION.into(), SECRET.into()),
                    (
                        PAYMENT_SIGNATURE.into(),
                        wire::encode_header(&self.payment).unwrap(),
                    ),
                ]),
                NOW,
            )
            .0
    }
    fn recovery(&self, front: &Front<FileReplayStore>, secret: &str) -> Response {
        front
            .handle(
                &self.request(vec![
                    (AUTHORIZATION.into(), secret.into()),
                    (PAYMENT.into(), self.hash.clone()),
                ]),
                NOW + 10_000,
            )
            .0
    }
    fn view(&self, front: &Front<FileReplayStore>) -> View {
        let r = self.recovery(front, SECRET);
        assert_eq!(r.status, 200);
        serde_json::from_slice(&r.body).unwrap()
    }
    fn prepare(&self) -> Transaction {
        let request_hash = binding_hash(
            &http_binding(
                "POST",
                "https://fixture.invalid/v1/plugins/notes/invoke",
                &self.body,
                &[],
            )
            .unwrap(),
        )
        .unwrap();
        let settlement = Settlement {
            payment_hash: self.hash.clone(),
            request_hash: request_hash.clone(),
            route: "invoke".into(),
            resource: "fixture".into(),
            role: "plugin_call".into(),
            plugin: self.quote.plugin.clone(),
            release: self.quote.release.clone(),
            author: self.quote.author.clone(),
            fee_msat: self.quote.fee_msat,
            price_msat: 6000,
            received_msat: 6000,
            received_from_wallet: true,
            scheme: front::Scheme::X402,
            network: MAINNET.into(),
            settled_at: NOW,
        };
        Store::open(&self.root.path().join("outcomes"))
            .unwrap()
            .prepare(
                Identity {
                    network: MAINNET.into(),
                    payment_hash: self.hash.clone(),
                    invoice: self.payment.accepted.extra["invoice"]
                        .as_str()
                        .unwrap()
                        .into(),
                    request_hash: request_hash.clone(),
                    authorization: commitment(SECRET),
                    quote: self.quote.clone(),
                },
                ReplayEntry {
                    key: format!("{MAINNET}:{}", self.hash),
                    network: MAINNET.into(),
                    payment_hash: self.hash.clone(),
                    amount_msat: 6000,
                    consumed_at: NOW,
                    retain_until: u64::MAX,
                    purchase: format!("invoke:{request_hash}"),
                },
                settlement,
                wire::SettlementResponse {
                    success: true,
                    error_reason: None,
                    transaction: self.hash.clone(),
                    network: MAINNET.into(),
                    amount: Some("6000".into()),
                },
            )
            .unwrap()
    }
    fn settled_count(&self) -> usize {
        self.sink.ledger.lock().unwrap().since(0).unwrap().len()
    }
}

#[test]
fn retained_result_requires_original_secret_exact_body_and_survives_changed_current_quote() {
    let mut f = Fixture::new();
    let front = f.front(0);
    assert_eq!(f.invoke(&front).status, 200);
    let original_settlements = f.sink.ledger.lock().unwrap().since(0).unwrap();
    assert_eq!(f.invoke(&front).status, 409);
    assert_eq!(f.recovery(&front, &"b".repeat(64)).status, 403);
    let mut wrong = f.request(vec![
        (AUTHORIZATION.into(), SECRET.into()),
        (PAYMENT.into(), f.hash.clone()),
    ]);
    wrong.body.push(b' ');
    assert_eq!(front.handle(&wrong, NOW).0.status, 403);
    wrong = f.request(vec![
        (AUTHORIZATION.into(), SECRET.into()),
        (AUTHORIZATION.into(), SECRET.into()),
        (PAYMENT.into(), f.hash.clone()),
    ]);
    assert_eq!(front.handle(&wrong, NOW).0.status, 403);
    let first = f.view(&front);
    assert_eq!(first.stage, Stage::Completed);
    assert_eq!(
        serde_json::from_slice::<Value>(&first.response.unwrap().body).unwrap()["private_result"],
        "Only the original purchase secret can retrieve this"
    );
    drop(front);
    f.quote.release = Some("d".repeat(64));
    f.quote.price_msat = 9000;
    let restarted = f.front(0);
    let recovered = f.view(&restarted);
    assert_eq!(recovered.identity.quote.price_msat, 6000);
    assert_eq!(recovered.stage, Stage::Completed);
    assert_eq!(f.count.load(Ordering::SeqCst), 1);
    assert_eq!(f.settled_count(), 1);
    assert_eq!(
        f.sink.ledger.lock().unwrap().since(0).unwrap(),
        original_settlements
    );
}

#[test]
fn known_guest_failure_and_interrupted_invocation_remain_distinct_after_restart() {
    for mode in [1, 2, 3] {
        let f = Fixture::new();
        let front = f.front(mode);
        let result = catch_unwind(AssertUnwindSafe(|| f.invoke(&front)));
        if mode == 2 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().status, if mode == 1 { 500 } else { 503 });
        }
        if mode == 3 {
            for e in std::fs::read_dir(f.root.path().join("outcomes")).unwrap() {
                let p = e.unwrap().path();
                if p.extension().is_some_and(|s| s == "pending") {
                    std::fs::remove_dir(p).unwrap();
                }
            }
        }
        drop(front);
        let front = f.front(0);
        let recovered = f.view(&front);
        assert_eq!(
            recovered.stage,
            if mode == 1 {
                Stage::Failed
            } else {
                Stage::Invoking
            }
        );
        assert_eq!(recovered.response.is_some(), mode == 1);
        assert!(recovered.settlement.is_some());
        assert_eq!(f.invoke(&front).status, 409);
        assert_eq!(f.count.load(Ordering::SeqCst), 1);
        assert_eq!(f.settled_count(), 1);
    }
}

#[test]
fn interrupted_payment_replay_settlement_and_pre_invocation_boundaries_never_run_on_recovery() {
    for boundary in [
        "admission",
        "replay",
        "ledger-append",
        "settled",
        "invoking",
    ] {
        let f = Fixture::new();
        let front = f.front(0);
        let mut tx = f.prepare();
        if boundary != "admission" {
            FileReplayStore::open(&f.root.path().join("replay"))
                .unwrap()
                .insert(&tx.record.replay)
                .unwrap();
        }
        if matches!(boundary, "ledger-append" | "settled" | "invoking") {
            tx.stage(Stage::SettlementPending).unwrap();
            f.sink.on_settled(&tx.record.settlement).unwrap();
        }
        if matches!(boundary, "settled" | "invoking") {
            tx.stage(Stage::Settled).unwrap();
        }
        if boundary == "invoking" {
            tx.stage(Stage::Invoking).unwrap();
        }
        drop(tx);
        drop(front);
        let front = f.front(0);
        let a = f.view(&front);
        let b = f.view(&front);
        assert_eq!(
            a.stage,
            if boundary == "invoking" {
                Stage::Invoking
            } else {
                Stage::Failed
            },
            "{boundary}"
        );
        assert_eq!(a.receipt_reference, b.receipt_reference);
        assert_eq!(a.response.is_none(), boundary == "invoking");
        assert_eq!(f.invoke(&front).status, 409);
        assert_eq!(f.count.load(Ordering::SeqCst), 0);
        assert_eq!(f.settled_count(), 1);
    }
}

#[test]
fn unavailable_or_wrong_wallet_observation_retains_prepared_liability() {
    let f = Fixture::new();
    let tx = f.prepare();
    drop(tx);
    let front = f.front(0);
    f.wallet.received.lock().unwrap().clear();
    assert_eq!(f.view(&front).stage, Stage::Prepared);
    f.wallet
        .received
        .lock()
        .unwrap()
        .insert(f.hash.clone(), 7000);
    assert_eq!(f.view(&front).stage, Stage::Prepared);
    assert_eq!(f.settled_count(), 0);
    assert_eq!(f.count.load(Ordering::SeqCst), 0);
    f.wallet
        .received
        .lock()
        .unwrap()
        .insert(f.hash.clone(), 6000);
    assert_eq!(f.view(&front).stage, Stage::Failed);
    assert_eq!(f.settled_count(), 1);
}

#[test]
fn settlement_loss_and_append_before_crash_reconcile_through_central_idempotency() {
    for mode in [1, 2] {
        let f = Fixture::new();
        let front = f.front(0);
        f.sink.fail.store(mode, Ordering::SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| f.invoke(&front)));
        if mode == 1 {
            assert_eq!(result.unwrap().status, 503);
        } else {
            assert!(result.is_err());
        }
        f.sink.fail.store(0, Ordering::SeqCst);
        assert_eq!(f.view(&front).stage, Stage::Failed);
        assert_eq!(f.view(&front).stage, Stage::Failed);
        assert_eq!(f.settled_count(), 1);
        assert_eq!(f.count.load(Ordering::SeqCst), 0);
    }
}

#[derive(Serialize, Deserialize)]
struct ProcessInput {
    root: std::path::PathBuf,
    body: Vec<u8>,
    hash: String,
    quote: Quote,
    payment: PaymentPayload,
    mode: String,
}

#[test]
fn process_worker() {
    let Some(path) = std::env::var_os("OPENAGENTS_REV12_PROCESS_FIXTURE") else {
        return;
    };
    let input: ProcessInput = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let root = Root {
        path: input.root.clone(),
        _owner: None,
    };
    let wallet = Arc::new(Wallet {
        counter: AtomicU64::new(0),
        preimages: Mutex::new(BTreeMap::new()),
        received: Mutex::new(BTreeMap::from([(input.hash.clone(), 6000)])),
    });
    let sink = Arc::new(Sink {
        ledger: Mutex::new(pay_ledger::Ledger::open(&root.path().join("ledger.sqlite")).unwrap()),
        fail: AtomicU64::new(if input.mode == "settlement" { 3 } else { 0 }),
        pause: Some(root.path().join("ready")),
    });
    let f = Fixture {
        root,
        wallet,
        sink,
        count: Arc::new(AtomicU64::new(0)),
        quote: input.quote,
        body: input.body,
        hash: input.hash,
        payment: input.payment,
    };
    let front = f.front(if input.mode == "invocation" { 4 } else { 0 });
    assert_eq!(f.invoke(&front).status, 200);
    assert_eq!(input.mode, "result");
    File::create(input.root.join("ready"))
        .unwrap()
        .sync_all()
        .unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[test]
fn killed_native_process_retains_exact_settlement_invocation_and_result_custody() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for mode in ["settlement", "invocation", "result"] {
        let f = Fixture::new();
        let input = ProcessInput {
            root: f.root.path().to_path_buf(),
            body: f.body.clone(),
            hash: f.hash.clone(),
            quote: f.quote.clone(),
            payment: f.payment.clone(),
            mode: mode.into(),
        };
        let path = f.root.path().join("process.json");
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        let mut child = Child(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "outcome::tests::process_worker", "--nocapture"])
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", f.root.path().join("fake-home"))
                .env("OPENAGENTS_REV12_PROCESS_FIXTURE", path)
                .current_dir(f.root.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !f.root.path().join("ready").is_file() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "Synthetic child stopped before {mode} boundary."
            );
            assert!(
                std::time::Instant::now() < until,
                "Synthetic {mode} boundary timed out."
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let restarted = f.front(0);
        let first = f.view(&restarted);
        let again = f.view(&restarted);
        assert_eq!(
            first.stage,
            match mode {
                "settlement" => Stage::Failed,
                "invocation" => Stage::Invoking,
                _ => Stage::Completed,
            }
        );
        assert_eq!(first.receipt_reference, again.receipt_reference);
        assert_eq!(first.response.is_some(), mode != "invocation");
        assert_eq!(f.settled_count(), 1);
        assert_eq!(f.count.load(Ordering::SeqCst), 0);
        assert_eq!(f.invoke(&restarted).status, 409);
    }
}

#[test]
fn concurrent_paid_proof_and_status_cannot_redispatch_an_inflight_guest() {
    let f = Fixture::new();
    let base = f.front(0);
    let mut route = base.routes()[0].clone();
    let count = f.count.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let waiting = gate.clone();
    route.executor = Arc::new(move |_: &Call<'_>| {
        count.fetch_add(1, Ordering::SeqCst);
        tx.send(()).unwrap();
        let (lock, wake) = &*waiting;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = wake.wait(released).unwrap();
        }
        Ok(front::Output {
            body: b"{}".to_vec(),
            content_type: Some("application/json".into()),
        })
    });
    let front = Arc::new(
        Front::new(
            Config {
                base_url: "https://fixture.invalid".into(),
                network: MAINNET,
                realm: "fixture".into(),
                challenge_key: vec![7; 32],
                timeout_secs: 300,
            },
            f.wallet.clone(),
            Facilitator::new(
                FileReplayStore::open(&f.root.path().join("replay")).unwrap(),
                60,
            ),
            f.sink.clone(),
            vec![route],
        )
        .unwrap()
        .with_outcomes(Store::open(&f.root.path().join("outcomes")).unwrap()),
    );
    let request = f.request(vec![
        (AUTHORIZATION.into(), SECRET.into()),
        (
            PAYMENT_SIGNATURE.into(),
            wire::encode_header(&f.payment).unwrap(),
        ),
    ]);
    let worker_front = front.clone();
    let worker = std::thread::spawn(move || worker_front.handle(&request, NOW).0);
    rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(f.invoke(&front).status, 409);
    assert_eq!(f.recovery(&front, SECRET).status, 403);
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    assert_eq!(worker.join().unwrap().status, 200);
    assert_eq!(f.view(&front).stage, Stage::Completed);
    assert_eq!(f.count.load(Ordering::SeqCst), 1);
    assert_eq!(f.settled_count(), 1);
}

#[test]
fn copied_paid_proof_without_original_secret_and_second_invoice_never_admit_an_execution() {
    let f = Fixture::new();
    let front = f.front(0);
    let copied = f.request(vec![(
        PAYMENT_SIGNATURE.into(),
        wire::encode_header(&f.payment).unwrap(),
    )]);
    assert_eq!(front.handle(&copied, NOW).0.status, 403);
    assert_eq!(f.count.load(Ordering::SeqCst), 0);
    assert_eq!(f.settled_count(), 0);
    // Two unpaid quotes can race. Only the first exact invoice may bind this purchase.
    let response = front.handle(&f.request(vec![]), NOW).0;
    assert_eq!(response.status, 402);
    let required = wire::decode_payment_required(
        response
            .headers
            .iter()
            .find(|(n, _)| n == PAYMENT_REQUIRED)
            .unwrap()
            .1
            .as_str(),
    )
    .unwrap();
    let mut second = f.payment.clone();
    second.accepted = required.accepts[0].clone();
    let invoice =
        nostr::x402::decode_invoice(second.accepted.extra["invoice"].as_str().unwrap()).unwrap();
    let hash = hex::encode(invoice.payment_hash());
    assert_ne!(hash, f.hash);
    second.payload.insert(
        "preimage".into(),
        json!(hex::encode(f.wallet.preimages.lock().unwrap()[&hash])),
    );
    assert_eq!(f.invoke(&front).status, 200);
    let request = f.request(vec![
        (AUTHORIZATION.into(), SECRET.into()),
        (
            PAYMENT_SIGNATURE.into(),
            wire::encode_header(&second).unwrap(),
        ),
    ]);
    assert_eq!(front.handle(&request, NOW).0.status, 409);
    assert_eq!(front.handle(&f.request(vec![]), NOW).0.status, 409);
    assert_eq!(f.count.load(Ordering::SeqCst), 1);
    assert_eq!(f.settled_count(), 1);
}

#[test]
fn receipt_projection_is_bounded_and_private_custody_refuses_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = Fixture::new();
    let front = f.front(0);
    assert_eq!(f.invoke(&front).status, 200);
    let dir = f.root.path().join("outcomes");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o077,
            0
        );
        if p.extension().is_some_and(|s| s == "json") {
            let saved = std::fs::read(&p).unwrap();
            assert!(!String::from_utf8_lossy(&saved).contains(SECRET));
            let elsewhere = f.root.path().join("substitution");
            std::fs::write(&elsewhere, saved).unwrap();
            std::fs::remove_file(&p).unwrap();
            symlink(elsewhere, p).unwrap();
        }
    }
    assert_eq!(f.recovery(&front, SECRET).status, 403);
}

#[test]
fn initial_delivery_waits_for_receiver_collection_and_recovery_never_executes_it() {
    let f = Fixture::new();
    f.wallet.received.lock().unwrap().clear();
    let front = f.front(0);
    assert_eq!(f.invoke(&front).status, 503);
    assert_eq!(f.view(&front).stage, Stage::Prepared);
    assert_eq!(f.count.load(Ordering::SeqCst), 0);
    assert_eq!(f.settled_count(), 0);
    f.wallet
        .received
        .lock()
        .unwrap()
        .insert(f.hash.clone(), 5800);
    assert_eq!(f.view(&front).stage, Stage::Failed);
    assert_eq!(f.count.load(Ordering::SeqCst), 0);
    assert_eq!(f.settled_count(), 1);
    assert_eq!(f.invoke(&front).status, 409);
    let recorded = f.sink.ledger.lock().unwrap().since(0).unwrap();
    assert_eq!(recorded[0].price_msat, 6000);
    assert_eq!(recorded[0].received_msat, 5800);
    assert_eq!(recorded[0].lsp_fee_msat, 200);
    assert!(
        recorded[0]
            .shares
            .iter()
            .any(|s| s.role == "author" && s.amount_msat == 1000)
    );
}

#[test]
fn settlement_journal_deduplicates_across_instances_and_refuses_partial_or_conflicting_evidence() {
    use std::io::Write;
    let f = Fixture::new();
    let settlement = f.prepare().record.settlement.clone();
    let path = f.root.path().join("settlements.ndjson");
    let first = front::NdjsonSettlements::open(&path).unwrap();
    first.on_settled(&settlement).unwrap();
    front::NdjsonSettlements::open(&path)
        .unwrap()
        .on_settled(&settlement)
        .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
    let mut changed = settlement.clone();
    changed.received_msat += 1;
    assert!(first.on_settled(&changed).is_err());
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{partial")
        .unwrap();
    assert!(first.on_settled(&settlement).is_err());
}

#[test]
fn private_outcome_refuses_shared_hard_links_and_changed_signed_invoice() {
    let f = Fixture::new();
    let front = f.front(0);
    assert_eq!(f.invoke(&front).status, 200);
    let record = std::fs::read_dir(f.root.path().join("outcomes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let alias = f.root.path().join("shared-result");
    std::fs::hard_link(&record, &alias).unwrap();
    assert_eq!(f.recovery(&front, SECRET).status, 403);
    std::fs::remove_file(alias).unwrap();
    assert_eq!(f.view(&front).stage, Stage::Completed);
    let g = Fixture::new();
    let mut tx = g.prepare();
    let mut changed = tx.record.identity.invoice.clone();
    changed.push('x');
    tx.record.identity.invoice = changed;
    assert!(tx.stage(Stage::SettlementPending).is_err());
}

#[test]
fn custody_replacement_during_collection_cannot_overwrite_recovery_or_admit_execution() {
    use std::{io::Write, os::unix::fs::PermissionsExt};
    struct Paused {
        wallet: Arc<Wallet>,
        started: std::sync::mpsc::Sender<()>,
        gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
    }
    impl Receiver for Paused {
        fn pay_to(&self) -> String {
            self.wallet.pay_to()
        }
        fn invoice(&self, a: u64, h: [u8; 32], e: u32) -> Result<String, String> {
            self.wallet.invoice(a, h, e)
        }
        fn received_msat(&self, h: [u8; 32]) -> Result<Option<u64>, String> {
            self.started.send(()).unwrap();
            let mut ready = self.gate.0.lock().unwrap();
            while !*ready {
                ready = self.gate.1.wait(ready).unwrap();
            }
            self.wallet.received_msat(h)
        }
    }
    struct Release(Arc<(Mutex<bool>, std::sync::Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap() = true;
            self.0.1.notify_all();
        }
    }
    for mutation in [
        "payment-lock",
        "source-lock",
        "both-locks",
        "root",
        "record",
        "record-bytes",
        "binding",
    ] {
        let f = Fixture::new();
        let base = f.front(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let release = Release(gate.clone());
        let front = Arc::new(
            Front::new(
                Config {
                    base_url: "https://fixture.invalid".into(),
                    network: MAINNET,
                    realm: "fixture".into(),
                    challenge_key: vec![7; 32],
                    timeout_secs: 300,
                },
                Arc::new(Paused {
                    wallet: f.wallet.clone(),
                    started: tx,
                    gate,
                }),
                Facilitator::new(
                    FileReplayStore::open(&f.root.path().join("replay")).unwrap(),
                    60,
                ),
                f.sink.clone(),
                base.routes().to_vec(),
            )
            .unwrap()
            .with_outcomes(Store::open(&f.root.path().join("outcomes")).unwrap()),
        );
        let request = f.request(vec![
            (AUTHORIZATION.into(), SECRET.into()),
            (
                PAYMENT_SIGNATURE.into(),
                wire::encode_header(&f.payment).unwrap(),
            ),
        ]);
        let running = front.clone();
        let worker = std::thread::spawn(move || running.handle(&request, NOW).0);
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let directory = f.root.path().join("outcomes");
        let files = std::fs::read_dir(&directory)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        let payment_lock = files
            .iter()
            .find(|p| {
                p.extension().is_some_and(|e| e == "lock")
                    && !p
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("purchase-")
            })
            .unwrap();
        let source_lock = files
            .iter()
            .find(|p| {
                p.extension().is_some_and(|e| e == "lock")
                    && p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("purchase-")
            })
            .unwrap();
        let record = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap();
        let binding = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "binding"))
            .unwrap();
        let replace = |path: &std::path::Path| {
            let bytes = std::fs::read(path).unwrap();
            std::fs::remove_file(path).unwrap();
            std::fs::write(path, bytes).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        };
        match mutation {
            "payment-lock" => replace(payment_lock),
            "source-lock" => replace(source_lock),
            "both-locks" => {
                replace(payment_lock);
                replace(source_lock);
            }
            "record" => replace(record),
            "binding" => replace(binding),
            "record-bytes" => std::fs::OpenOptions::new()
                .append(true)
                .open(record)
                .unwrap()
                .write_all(b" ")
                .unwrap(),
            _ => {
                std::fs::rename(&directory, f.root.path().join("detached-outcomes")).unwrap();
                std::fs::create_dir(&directory).unwrap();
                std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                    .unwrap();
            }
        }
        if mutation == "both-locks" {
            let recovery = f.front(0);
            assert_eq!(f.view(&recovery).stage, Stage::Failed);
            assert_eq!(f.settled_count(), 1);
        } else {
            assert_eq!(f.recovery(&front, SECRET).status, 403);
        }
        drop(release);
        assert_eq!(worker.join().unwrap().status, 503, "{mutation}");
        assert_eq!(f.count.load(Ordering::SeqCst), 0, "{mutation}");
        assert_eq!(
            f.settled_count(),
            usize::from(mutation == "both-locks"),
            "{mutation}"
        );
        if mutation == "both-locks" {
            assert_eq!(f.view(&f.front(0)).stage, Stage::Failed);
        }
    }
}
