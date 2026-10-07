use super::*;
use crate::pay_plugin::tests::{
    FakeReceiver, NOW, front_with, header, to_hex, useful_release::signed_source,
};
use openagents_wallet::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof, WalletError};
use openagents_x402::{FileReplayStore, front::Front, server::Request};
use receipts::purchase::{Context, PriceReference};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Wire {
    front: Front<FileReplayStore>,
    lost: AtomicBool,
    upgraded_verification: AtomicBool,
}
impl Transport for Wire {
    fn send(&self, url: &str, body: &[u8], signature: Option<&str>) -> Result<Reply, String> {
        let parsed = reqwest::Url::parse(url).unwrap();
        let request = Request {
            method: "POST".into(),
            target: parsed.path().into(),
            headers: signature
                .map(|s| vec![(PAYMENT_SIGNATURE.into(), s.into())])
                .unwrap_or_default(),
            body: body.to_vec(),
        };
        let (response, _) = self.front.handle(&request, NOW);
        if signature.is_some() && self.lost.load(Ordering::SeqCst) {
            return Err("Synthetic lost delivery acknowledgment.".into());
        }
        let required = header(&response, PAYMENT_REQUIRED).map(str::to_owned);
        let settlement = header(&response, PAYMENT_RESPONSE).map(str::to_owned);
        let mut body = response.body;
        if signature.is_some() && self.upgraded_verification.load(Ordering::SeqCst) {
            let mut result: Value = serde_json::from_slice(&body).unwrap();
            result["verification"] = json!("exact_replay");
            body = serde_json::to_vec(&result).unwrap();
        }
        Ok(Reply {
            status: response.status,
            required,
            settlement,
            body,
        })
    }
}
struct Wallet {
    receiver: Arc<FakeReceiver>,
    payments: AtomicU64,
    pending: AtomicBool,
}
impl LightningWallet for Wallet {
    fn node_id(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of([19; 32]))
    }
    fn pay(&self, invoice: &str, fee: u64, _: Duration) -> Result<Proof, WalletError> {
        assert_eq!(fee, 0);
        self.payments.fetch_add(1, Ordering::SeqCst);
        let parsed = nostr::x402::decode_invoice(invoice).unwrap();
        if self.pending.load(Ordering::SeqCst) {
            return Err(WalletError::Pending {
                payment_hash: to_hex(parsed.payment_hash()),
                waited_secs: 1,
            });
        }
        Ok(Proof {
            payment_hash: to_hex(parsed.payment_hash()),
            preimage: self.receiver.pay(invoice),
            amount_msat: parsed.amount_msat(),
            fee_msat: 0,
            bolt11: invoice.into(),
        })
    }
    fn receive_exact(&self, _: u64, _: [u8; 32], _: u32) -> Result<IssuedInvoice, WalletError> {
        unreachable!()
    }
    fn lookup(&self, _: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        unreachable!()
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        unreachable!()
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        unreachable!()
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        unreachable!()
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        unreachable!()
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        unreachable!()
    }
}
impl openagents_wallet::resident::Served for Wallet {
    fn status(&self) -> Value {
        json!({"network":"bitcoin","running":true})
    }
    fn buy_channel(&self, _: u64, _: u64, _: u32, _: bool) -> Result<Value, WalletError> {
        unreachable!()
    }
    fn channel_order(&self, _: &str) -> Result<Value, WalletError> {
        unreachable!()
    }
    fn send_onchain(&self, _: &str, _: u64) -> Result<String, WalletError> {
        unreachable!()
    }
}
fn current() -> Selection {
    let hash = |c: char| format!("sha256:{}", c.to_string().repeat(64));
    Selection {
        origin: "https://api.example.com".into(),
        credential_alias: "buyer".into(),
        context: Context {
            schema: receipts::purchase::SCHEMA.into(),
            account: "buyer".into(),
            workspace: "buyer-workspace".into(),
            payer_workspace: "buyer-workspace".into(),
            tenant: "buyer-tenant".into(),
            credential_reference: "buyer-key".into(),
            membership_epoch: 1,
            workspace_members_epoch: 1,
            role: "owner".into(),
            door: "decision-a".into(),
            registry_digest: hash('a'),
            artifact_digest: hash('b'),
            price: PriceReference {
                version: "price-1".into(),
                currency: "USD".into(),
                policy: "observed-usage-v1".into(),
                terms_digest: hash('c'),
                maximum_usage_digest: hash('d'),
                maximum_charge: 100,
            },
            can_invoke: true,
        },
    }
}
struct Harness {
    root: tempfile::TempDir,
    store: Store,
    current: Selection,
    offer: Offer,
    wire: Wire,
    wallet: Wallet,
    ledger: Ledger,
}
impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let (source, id) = signed_source(root.path());
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(Default::default()),
        });
        let sink = Arc::new(pay_plugin::LedgerSink::in_memory());
        let wire = Wire {
            front: front_with(root.path(), receiver.clone(), sink, source.clone()),
            lost: AtomicBool::new(false),
            upgraded_verification: AtomicBool::new(false),
        };
        let wallet = Wallet {
            receiver,
            payments: AtomicU64::new(0),
            pending: AtomicBool::new(false),
        };
        let current = current();
        let mut store = Store::open(&root.path().join("customer")).unwrap();
        store
            .import_credential("buyer", &jev::ApiKey::new("oak_fixture.buyer"))
            .unwrap();
        store.bind(current.clone()).unwrap();
        let url = format!("{}/v1/plugins/{id}/invoke", current.origin);
        let request = include_str!("../../../../plugins/meeting-action-items/examples/meeting.md");
        let quote = preview(&wire, &url).unwrap();
        let mut offer = Offer {
            url,
            relay: "ws://fixture.invalid".into(),
            blossom: None,
            quote,
            payment: wire::PaymentRequired {
                x402_version: 2,
                error: None,
                resource: wire::ResourceInfo {
                    url: String::new(),
                    description: None,
                    mime_type: None,
                    rest: Default::default(),
                },
                accepts: vec![],
                extensions: None,
            },
            packet: Packet {
                module: String::new(),
                input: String::new(),
                operation: String::new(),
                profile: String::new(),
                limits: Value::Null,
            },
            payer: Payer {
                home: root.path().join("wallet"),
                node: wallet.node_id(),
                network: nostr::x402::MAINNET.into(),
            },
            max_msat: 6000,
            max_fee_msat: 0,
            request_hash: String::new(),
            expires_at_ms: (NOW + 300) * 1000,
        };
        offer.packet = resolved(source.as_ref(), &offer, request).unwrap();
        let body = offer.body(request);
        offer.request_hash =
            binding_hash(&http_binding("POST", &offer.url, &body, &[]).unwrap()).unwrap();
        let reply = wire.send(&offer.url, &body, None).unwrap();
        assert_eq!(reply.status, 402);
        offer.payment = wire::decode_payment_required(reply.required.as_ref().unwrap()).unwrap();
        store
            .quote_plugin(
                "one",
                offer.clone(),
                request.into(),
                current.clone(),
                NOW * 1000,
            )
            .unwrap();
        let ledger = Ledger::open(&root.path().join("buyer.ndjson"));
        Self {
            root,
            store,
            current,
            offer,
            wire,
            wallet,
            ledger,
        }
    }
    fn approve(&mut self) {
        let digest = self.store.plugin_view("one").unwrap().approval_digest;
        self.store
            .approve_plugin(
                "one",
                &digest,
                &self.current,
                &self.offer.payer,
                NOW * 1000 + 1,
            )
            .unwrap();
    }
    fn buy(&mut self) -> Result<View, String> {
        buy(
            &mut self.store,
            "one",
            &self.current,
            &self.offer.payer,
            &self.offer.packet,
            &self.wallet,
            &self.wire,
            None,
            0,
            &self.ledger,
            1,
            NOW * 1000 + 2,
        )
    }
}
#[test]
fn approved_signed_release_returns_useful_result_and_exact_charge_without_repayment() {
    let mut h = Harness::new();
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 0);
    h.approve();
    let v = h.buy().unwrap();
    assert_eq!(v.phase, Phase::Completed);
    assert_eq!(v.charge.as_ref().unwrap().amount_msat, 6000);
    assert_eq!(v.charge.as_ref().unwrap().fee_msat, 0);
    let result = v.result.unwrap();
    assert_eq!(result["value"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(result["value"]["items"][0]["owner"], "Ana");
    assert_eq!(result["verification"], "not_run");
    assert_eq!(h.ledger.entries().unwrap().len(), 1);
    assert_eq!(h.ledger.entries().unwrap()[0].phase, "http_200");
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    let mut h = Harness::new();
    h.approve();
    h.wire.upgraded_verification.store(true, Ordering::SeqCst);
    let v = h.buy().unwrap();
    assert_eq!(v.phase, Phase::Unknown);
    assert_eq!(v.charge.unwrap().amount_msat, 6000);
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
}
#[test]
fn changed_customer_payer_packet_expiry_and_cancellation_never_dispatch_payment() {
    for mode in ["rights", "payer", "packet", "expiry", "cancel"] {
        let mut h = Harness::new();
        h.approve();
        match mode {
            "rights" => h.current.context.can_invoke = false,
            "payer" => h.offer.payer.node = to_hex(nostr::x402::test_invoice::payee_of([20; 32])),
            "packet" => h.offer.packet.input = plugin::digest(b"changed input"),
            "cancel" => {
                h.store.cancel_plugin("one").unwrap();
            }
            _ => {}
        }
        let at = if mode == "expiry" {
            (NOW + 300) * 1000
        } else {
            NOW * 1000 + 2
        };
        assert!(
            buy(
                &mut h.store,
                "one",
                &h.current,
                &h.offer.payer,
                &h.offer.packet,
                &h.wallet,
                &h.wire,
                None,
                0,
                &h.ledger,
                1,
                at
            )
            .is_err(),
            "{mode}"
        );
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 0);
        assert_eq!(h.ledger.entries().unwrap().len(), 0);
    }
}
#[test]
fn uncertain_payment_and_lost_delivery_keep_original_identity_after_restart() {
    for lost in [false, true] {
        let mut h = Harness::new();
        h.approve();
        h.wallet.pending.store(!lost, Ordering::SeqCst);
        h.wire.lost.store(lost, Ordering::SeqCst);
        let v = h.buy().unwrap();
        assert_eq!(v.phase, Phase::Unknown);
        assert_eq!(v.charge.is_some(), lost);
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
        assert!(h.buy().is_err());
        let dir = h.root.path().join("customer");
        drop(h.store);
        let mut store = Store::open(&dir).unwrap();
        let v = store.plugin_view("one").unwrap();
        assert_eq!(v.phase, Phase::Unknown);
        assert_eq!(v.charge.is_some(), lost);
        assert!(
            store
                .begin_plugin(
                    "one",
                    &h.current,
                    &h.offer.payer,
                    &h.offer.packet,
                    NOW * 1000 + 3
                )
                .is_err()
        );
        assert!(
            store
                .quote_plugin(
                    "replacement",
                    h.offer.clone(),
                    include_str!("../../../../plugins/meeting-action-items/examples/meeting.md")
                        .into(),
                    h.current.clone(),
                    NOW * 1000 + 3
                )
                .is_err()
        );
        let mut other = h.current.clone();
        other.context.account = "another-account".into();
        other.context.workspace = "another-workspace".into();
        other.context.payer_workspace = "another-workspace".into();
        store.bind(other.clone()).unwrap();
        assert!(
            store
                .quote_plugin(
                    "another-account",
                    h.offer.clone(),
                    include_str!("../../../../plugins/meeting-action-items/examples/meeting.md")
                        .into(),
                    other,
                    NOW * 1000 + 3,
                )
                .is_err()
        );
    }
}

struct LiveReceiver(Arc<FakeReceiver>);
impl openagents_x402::server::Receiver for LiveReceiver {
    fn pay_to(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of([9; 32]))
    }
    fn invoice(&self, amount: u64, hash: [u8; 32], expiry: u32) -> Result<String, String> {
        self.0
            .invoice_at(amount, hash, expiry, openagents_x402::unix_now())
    }
    fn received_msat(&self, _: [u8; 32]) -> Result<Option<u64>, String> {
        Ok(None)
    }
}

/// Run with OPENAGENTS_PLUGIN_CLI set to this checkout's freshly built binary.
#[test]
fn installed_cli_quotes_approves_pays_returns_and_preserves_unknown_delivery() {
    let Some(binary) = std::env::var_os("OPENAGENTS_PLUGIN_CLI") else {
        return;
    };
    use openagents_x402::{
        Facilitator,
        front::{Config, Route},
        server::{Response, serve_with},
    };
    use std::{net::TcpListener, os::unix::fs::PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let (source, id, events, blobs) =
        crate::pay_plugin::tests::useful_release::served_source(root.path());
    let stop = Arc::new(AtomicBool::new(false));
    let (relay, relay_thread) = relay_fixture(events, stop.clone());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let receiver = Arc::new(FakeReceiver {
        counter: AtomicU64::new(0),
        preimages: Mutex::new(Default::default()),
    });
    let wallet = Arc::new(Wallet {
        receiver: receiver.clone(),
        payments: AtomicU64::new(0),
        pending: AtomicBool::new(false),
    });
    let invoke = pay_plugin::Invoke::new(5000, source);
    let route = Route {
        id: "plugin-invoke".into(),
        method: "POST".into(),
        path: "/v1/plugins/{id}/invoke".into(),
        price: invoke.price(),
        executor: invoke,
        role: pay_plugin::ROLE.into(),
        resource: "plugin-invoke".into(),
        plugin: None,
        description: "Synthetic supplied-note action items".into(),
        mime_type: "application/json".into(),
        model_cost_only: false,
    };
    let front = Front::new(
        Config {
            base_url: origin.clone(),
            network: nostr::x402::MAINNET,
            realm: origin.clone(),
            challenge_key: vec![7; 32],
            timeout_secs: 300,
        },
        Arc::new(LiveReceiver(receiver.clone())),
        Facilitator::new(
            FileReplayStore::open(&root.path().join("replay")).unwrap(),
            60,
        ),
        Arc::new(pay_plugin::LedgerSink::in_memory()),
        vec![route],
    )
    .unwrap();
    let mut selected = current();
    selected.origin = origin.clone();
    let context = serde_json::to_value(&selected.context).unwrap();
    let loss = Arc::new(AtomicBool::new(false));
    let dropping = loss.clone();
    let denied = Arc::new(AtomicBool::new(false));
    let denial = denied.clone();
    let changed = Arc::new(AtomicBool::new(false));
    let changing = changed.clone();
    let http_stop = stop.clone();
    let http = std::thread::spawn(move || {
        serve_with(listener, http_stop, move |request| {
            if request.target == "/v1/workspaces/buyer-workspace/purchase-context/decision-a" {
                assert_eq!(
                    request.header("authorization"),
                    Some("Bearer oak_fixture.buyer")
                );
                let mut c = context.clone();
                c["can_invoke"] = json!(!denial.load(Ordering::SeqCst));
                return Response::json(200, &c);
            }
            if request.method == "GET" {
                let digest = format!("sha256:{}", request.target.trim_start_matches('/'));
                return blobs
                    .get(&digest)
                    .map(|b| Response {
                        status: 200,
                        headers: vec![],
                        body: b.clone(),
                    })
                    .unwrap_or_else(|| {
                        Response::json(404, &json!({"error":"missing synthetic blob"}))
                    });
            }
            let (mut response, _) = front.handle(request, openagents_x402::unix_now());
            if response.status == 409 && changing.load(Ordering::SeqCst) {
                let mut value: Value = serde_json::from_slice(&response.body).unwrap();
                let mut quote: Quote = serde_json::from_value(value["quote"].clone()).unwrap();
                quote.release = Some("f".repeat(64));
                value["quote"] = json!(quote);
                value["quote_digest"] = json!(execution::quote_digest(&quote));
                response = Response::json(409, &value);
            }
            if request.header(PAYMENT_SIGNATURE).is_some() && dropping.load(Ordering::SeqCst) {
                return Response::json(504, &json!({"error":"synthetic lost acknowledgment"}));
            }
            response
        })
        .unwrap()
    });
    let customer = root.path().join("customer");
    {
        let mut store = Store::open(&customer).unwrap();
        store
            .import_credential("buyer", &jev::ApiKey::new("oak_fixture.buyer"))
            .unwrap();
        store.bind(selected).unwrap();
    }
    let wallet_home = root.path().join("wallet");
    std::fs::create_dir(&wallet_home).unwrap();
    std::fs::set_permissions(&wallet_home, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config =
        openagents_wallet::WalletConfig::new(openagents_wallet::config::Network::Bitcoin, None)
            .unwrap();
    std::fs::write(
        wallet_home.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(
        wallet_home.join("config.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let resident = openagents_wallet::resident::Server::bind(&wallet_home).unwrap();
    let resident_stop = resident.stop_flag();
    let paying = wallet.clone();
    let wallet_thread = std::thread::spawn(move || resident.run(paying));
    let notes = root.path().join("notes.txt");
    std::fs::write(
        &notes,
        include_bytes!("../../../../plugins/meeting-action-items/examples/meeting.md"),
    )
    .unwrap();
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o600)).unwrap();
    let run = |words: &[&str]| {
        let output = std::process::Command::new(&binary)
            .args(["--json", "plugin", "purchase"])
            .args(words)
            .arg("--root")
            .arg(&customer)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .env("VERSE_HOME", root.path().join("identities"))
            .env("OPENAGENTS_TASKS", root.path().join("tasks"))
            .env("OPENAGENTS_X402_HOME", root.path().join("x402"))
            .current_dir(root.path())
            .output()
            .unwrap();
        let body = String::from_utf8(output.stdout.clone()).unwrap();
        assert!(!body.contains("oak_fixture.buyer"));
        (output, serde_json::from_str::<Value>(&body).unwrap())
    };
    let quote = |purchase: &str| {
        let (o, v) = run(&[
            "quote",
            "--purchase",
            purchase,
            "--plugin",
            &id,
            "--input",
            notes.to_str().unwrap(),
            "--wallet-home",
            wallet_home.to_str().unwrap(),
            "--max-msat",
            "6000",
            "--max-fee-msat",
            "0",
            "--relay",
            &relay,
            "--blossom",
            &origin,
        ]);
        assert!(
            o.status.success(),
            "{} {}",
            String::from_utf8_lossy(&o.stderr),
            v
        );
        v
    };
    let q = quote("one");
    assert_eq!(q["offer"]["quote"]["price_msat"], 6000);
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    assert!(
        run(&["approve", "--purchase", "one", "--digest", "edited"])
            .0
            .status
            .code()
            .is_some_and(|c| c != 0)
    );
    changed.store(true, Ordering::SeqCst);
    assert!(
        !run(&[
            "approve",
            "--purchase",
            "one",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    changed.store(false, Ordering::SeqCst);
    assert!(
        run(&[
            "approve",
            "--purchase",
            "one",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    denied.store(true, Ordering::SeqCst);
    assert!(!run(&["invoke", "--purchase", "one"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    denied.store(false, Ordering::SeqCst);
    let (o, v) = run(&["invoke", "--purchase", "one"]);
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stderr),
        v
    );
    assert_eq!(v["phase"], "completed");
    assert_eq!(v["charge"]["amount_msat"], 6000);
    assert_eq!(v["result"]["value"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(v["result"]["verification"], "not_run");
    assert_eq!(v["settlement"]["transaction"], v["charge"]["payment_hash"]);
    assert!(!run(&["invoke", "--purchase", "one"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 1);
    let q = quote("cancelled");
    assert!(
        run(&["cancel", "--purchase", "cancelled"])
            .0
            .status
            .success()
    );
    assert!(
        !run(&[
            "approve",
            "--purchase",
            "cancelled",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    assert!(
        !run(&["invoke", "--purchase", "cancelled"])
            .0
            .status
            .success()
    );
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 1);
    let q = quote("lost");
    assert!(
        run(&[
            "approve",
            "--purchase",
            "lost",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    loss.store(true, Ordering::SeqCst);
    let (o, v) = run(&["invoke", "--purchase", "lost"]);
    assert!(!o.status.success());
    assert_eq!(v["phase"], "unknown");
    assert_eq!(v["charge"]["amount_msat"], 6000);
    assert!(!run(&["invoke", "--purchase", "lost"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 2);
    let (o, v) = run(&["show", "--purchase", "lost"]);
    assert!(!o.status.success());
    assert_eq!(v["phase"], "unknown");
    assert_eq!(v["unresolved_maximum_msat"], 6000);
    stop.store(true, Ordering::SeqCst);
    resident_stop.store(true, Ordering::SeqCst);
    http.join().unwrap();
    wallet_thread.join().unwrap();
    relay_thread.join().unwrap();
}

fn relay_fixture(
    events: Vec<nostr::domain::Event>,
    stop: Arc<AtomicBool>,
) -> (String, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                use futures_util::{SinkExt, StreamExt};
                use tokio_tungstenite::tungstenite::Message;
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(format!("ws://{}", listener.local_addr().unwrap()))
                    .unwrap();
                while !stop.load(Ordering::SeqCst) {
                    let Ok(Ok((stream, _))) =
                        tokio::time::timeout(Duration::from_millis(50), listener.accept()).await
                    else {
                        continue;
                    };
                    let records = events.clone();
                    tokio::spawn(async move {
                        let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                            return;
                        };
                        let send = |v: Value| Message::Text(v.to_string().into());
                        socket.send(send(json!(["AUTH", "fixture"]))).await.unwrap();
                        while let Some(Ok(Message::Text(text))) = socket.next().await {
                            let value: Value = serde_json::from_str(&text).unwrap();
                            match value[0].as_str() {
                                Some("AUTH") => {
                                    let e: nostr::domain::Event =
                                        serde_json::from_value(value[1].clone()).unwrap();
                                    e.validate_crypto().unwrap();
                                    socket
                                        .send(send(json!(["OK", e.id, true, ""])))
                                        .await
                                        .unwrap();
                                }
                                Some("REQ") => {
                                    let filter = &value[2];
                                    for e in &records {
                                        let matches = ["ids", "authors", "kinds"].iter().all(|k| {
                                            filter[*k].as_array().is_none_or(|a| {
                                                a.iter().any(|v| match *k {
                                                    "ids" => v == &json!(e.id),
                                                    "authors" => v == &json!(e.pubkey),
                                                    _ => v == &json!(e.kind),
                                                })
                                            })
                                        }) && ["d", "t", "e"].iter().all(|k| {
                                            filter[format!("#{k}")].as_array().is_none_or(|a| {
                                                e.tag_values(k).any(|x| a.iter().any(|v| v == x))
                                            })
                                        });
                                        if matches {
                                            socket
                                                .send(send(json!(["EVENT", value[1], e])))
                                                .await
                                                .unwrap();
                                        }
                                    }
                                    socket.send(send(json!(["EOSE", value[1]]))).await.unwrap();
                                }
                                Some("CLOSE") => {}
                                _ => {}
                            }
                        }
                    });
                }
            })
    });
    (rx.recv().unwrap(), thread)
}
