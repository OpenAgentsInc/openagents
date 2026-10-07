//! Synthetic customers and collaborators only. No HOME, wallet node, Boat,
//! hosted model, or provider credential is read by these fixtures.

use super::*;
use pay_ledger::Ledger;
use retail_cloud::authority::Source;
use retail_cloud::cancel::{StopEvidence, StopOwner};
use retail_cloud::dispatch::{DispatchSpec, OwnerError, TaskEvent, TaskOwner, TaskStatus};
use retail_cloud::fake::{FakeProvider, FakeSandbox, FakeTaskOwner, FakeWallet};
use retail_cloud::material::Sandbox;
use retail_cloud::provision::{CreateSpec, ProviderError, Resource, ResourceState};
use retail_cloud::retain::{Artifact, Kind, Manifest};
use std::collections::BTreeMap;
use std::future::IntoFuture;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

const NOW: i64 = 1_791_200_000;
const CUSTOMER_KEY: &str = "synthetic-customer-model-key-never-live";
#[derive(Default)]
struct Runtime {
    provider: FakeProvider,
    sandbox: FakeSandbox,
    owner: FakeTaskOwner,
    specs: Mutex<BTreeMap<String, DispatchSpec>>,
    stops: Mutex<BTreeMap<String, StopEvidence>>,
    lost_stop: AtomicBool,
    clock: AtomicI64,
    stop_calls: AtomicU64,
    artifact_bytes: Mutex<Option<Vec<u8>>>,
    revoke_path: Mutex<Option<std::path::PathBuf>>,
}
impl Provider for Runtime {
    fn create(&self, spec: &CreateSpec) -> std::result::Result<Resource, ProviderError> {
        self.provider.create(spec)
    }
    fn find(&self, id: &str) -> std::result::Result<Option<Resource>, ProviderError> {
        self.provider.find(id)
    }
    fn state(&self, id: &str) -> std::result::Result<ResourceState, ProviderError> {
        if let Some(path) = self.revoke_path.lock().unwrap().take() {
            Ledger::open(path)
                .unwrap()
                .revoke_principal("cli:alice", NOW + 1)
                .unwrap();
        }
        self.provider.state(id)
    }
    fn delete(&self, id: &str) -> std::result::Result<(), ProviderError> {
        self.provider.delete(id)
    }
    fn usage_seconds(&self, id: &str) -> std::result::Result<Option<u64>, ProviderError> {
        self.provider.usage_seconds(id)
    }
}
impl Sandbox for Runtime {
    fn write_private(&self, r: &str, p: &str, c: &str) -> retail_cloud::Result<bool> {
        self.sandbox.write_private(r, p, c)
    }
    fn clone_source(&self, r: &str, s: &Source) -> retail_cloud::Result<(String, bool)> {
        self.sandbox.clone_source(r, s)
    }
    fn remove(&self, r: &str, p: &str) -> retail_cloud::Result<()> {
        self.sandbox.remove(r, p)
    }
    fn exists(&self, r: &str, p: &str) -> retail_cloud::Result<bool> {
        self.sandbox.exists(r, p)
    }
}
impl TaskOwner for Runtime {
    fn submit(&self, r: &str, s: &DispatchSpec) -> std::result::Result<(), OwnerError> {
        self.specs.lock().unwrap().insert(s.task.clone(), s.clone());
        self.owner.submit(r, s)
    }
    fn status(&self, r: &str, t: &str) -> std::result::Result<Option<TaskStatus>, OwnerError> {
        self.owner.status(r, t)
    }
    fn events(&self, r: &str, t: &str, c: u64) -> std::result::Result<Vec<TaskEvent>, OwnerError> {
        self.owner.events(r, t, c)
    }
}
impl StopOwner for Runtime {
    fn stop(&self, r: &str, t: &str, k: &str) -> std::result::Result<StopEvidence, OwnerError> {
        if let Some(e) = self.stops.lock().unwrap().get(k).cloned() {
            return Ok(e);
        }
        self.stop_calls.fetch_add(1, Ordering::SeqCst);
        let started = self.owner.status(r, t)?.is_some();
        self.owner.set_status(r, t, TaskStatus::Cancelled);
        let e = StopEvidence {
            at: if self.clock.load(Ordering::SeqCst) == 0 {
                NOW + 100
            } else {
                self.clock.load(Ordering::SeqCst)
            },
            started,
            status: TaskStatus::Cancelled,
            effects: vec![],
        };
        self.stops.lock().unwrap().insert(k.into(), e.clone());
        if self.lost_stop.swap(false, Ordering::SeqCst) {
            Err(OwnerError::Unknown("synthetic lost stop reply".into()))
        } else {
            Ok(e)
        }
    }
    fn stopped(
        &self,
        _r: &str,
        _t: &str,
        k: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError> {
        Ok(self.stops.lock().unwrap().get(k).cloned())
    }
}
impl Artifacts for Runtime {
    fn manifest(&self, r: &str, t: &str) -> retail_cloud::Result<Manifest> {
        let spec = self
            .specs
            .lock()
            .unwrap()
            .get(t)
            .cloned()
            .ok_or(retail_cloud::Error::Invalid("synthetic task absent"))?;
        let bytes = self
            .artifact_bytes
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| b"synthetic patch\n".to_vec());
        Ok(Manifest {
            execution: spec.execution,
            task: t.into(),
            resource: r.into(),
            source: Source {
                repository: "https://github.com/OpenAgentsInc/example".into(),
                commit: "c".repeat(40),
            },
            engine: "codex".into(),
            artifacts: vec![Artifact {
                name: "patch".into(),
                kind: Kind::Patch,
                digest: retail_cloud::sha256_hex(&bytes),
                size: bytes.len(),
            }],
        })
    }
    fn read(&self, _r: &str, _t: &str, n: &str, _max: usize) -> retail_cloud::Result<Vec<u8>> {
        if n == "patch" {
            Ok(self
                .artifact_bytes
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| b"synthetic patch\n".to_vec()))
        } else {
            Err(retail_cloud::Error::Invalid("synthetic artifact absent"))
        }
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    config: Config,
    runtime: Arc<Runtime>,
    wallet: Arc<FakeWallet>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        // This owner-record fixture opens the production gate structurally.
        // It is not a real funded qualification or deployment claim.
        let plan = retail_qualify::qualify::fixture();
        let mut receipt = retail_qualify::qualify::run_fake(&plan);
        receipt.mode = retail_qualify::qualify::Mode::Funded;
        receipt.label =
            "Synthetic owner-record gate fixture; no real funds or qualification.".into();
        let config = Config {
            schema: types::SCHEMA.into(),
            state: root.join("service"),
            ledger: root.join("ledger.sqlite"),
            template: "oa-coder-main-2026-10-07".into(),
            grants: vec![
                RetailGrant {
                    principal: "cli:alice".into(),
                    account: "alice".into(),
                    generation: 1,
                    observe: true,
                    execute: true,
                    disclose: true,
                },
                RetailGrant {
                    principal: "cli:bob".into(),
                    account: "bob".into(),
                    generation: 1,
                    observe: true,
                    execute: true,
                    disclose: true,
                },
                RetailGrant {
                    principal: "reader".into(),
                    account: "alice".into(),
                    generation: 1,
                    observe: true,
                    execute: false,
                    disclose: false,
                },
            ],
            contract_confirmed: true,
            qualification: Some(receipt),
            supported_plan: plan.digest(),
            plan_starts_left: Some(20),
        };
        let mut ledger = pay_ledger::Ledger::open(&config.ledger).unwrap();
        for account in ["alice", "bob"] {
            ledger.create_compute_account(account, NOW).unwrap();
            ledger
                .bind_principal(&pay_ledger::compute::Binding {
                    principal: format!("cli:{account}"),
                    account: account.into(),
                    kind: pay_ledger::compute::PrincipalKind::Cli,
                    credential: credential_digest(account),
                    rights: pay_ledger::compute::Rights {
                        read: true,
                        spend: true,
                    },
                    at: NOW,
                })
                .unwrap();
            let wallet = FakeWallet::new();
            let p = topup::request_top_up(
                &mut ledger,
                &wallet,
                &topup::TopUpRequest {
                    principal: format!("cli:{account}"),
                    credential: credential_digest(account),
                    purchase: format!("seed-{account}"),
                    amount_sats: 1000,
                    now: NOW,
                },
            )
            .unwrap();
            topup::on_paid(&mut ledger, &p.top_up.payment_hash, 1_000_000, NOW).unwrap();
        }
        ledger
            .bind_principal(&pay_ledger::compute::Binding {
                principal: "reader".into(),
                account: "alice".into(),
                kind: pay_ledger::compute::PrincipalKind::Cli,
                credential: credential_digest("reader"),
                rights: pay_ledger::compute::Rights {
                    read: true,
                    spend: false,
                },
                at: NOW,
            })
            .unwrap();
        Self {
            _temp: temp,
            config,
            runtime: Arc::new(Runtime::default()),
            wallet: Arc::new(FakeWallet::new()),
        }
    }
    fn open(&self) -> Service<Runtime, FakeWallet> {
        Service::open(
            self.config.clone(),
            self.runtime.clone(),
            self.wallet.clone(),
        )
        .unwrap()
    }
}
fn request() -> Value {
    json!({"source":{"repository":"https://github.com/OpenAgentsInc/example","commit":"c".repeat(40)},"task":"Fix trailing commas.","checks":["cargo test -p parser"],"max_seconds":600,"ceiling_sats":null})
}
fn call(s: &Service<Runtime, FakeWallet>, who: &str, value: Value) -> Result<Value> {
    s.call(
        &format!("cli:{who}"),
        who,
        serde_json::from_value(value).unwrap(),
        NOW,
    )
}
fn made(s: &Service<Runtime, FakeWallet>, who: &str, id: &str) -> Value {
    call(
        s,
        who,
        json!({"op":"offer","idempotency":id,"task":request()}),
    )
    .unwrap()["result"]
        .clone()
}
fn confirmation(m: &Value) -> Value {
    json!({"op":"confirm","offer":m["offer"]["id"],"digest":m["offer"]["digest"],"admission":digest_of(&m["admission"]),"custody":m["custody"]["digest"],"credential":{"provider":"openai","key":CUSTOMER_KEY,"service_custody":true}})
}
fn accepted(s: &Service<Runtime, FakeWallet>, who: &str, id: &str) -> String {
    let m = made(s, who, id);
    call(s, who, confirmation(&m)).unwrap()["result"]["execution"]
        .as_str()
        .unwrap()
        .into()
}
fn dispatch(s: &Service<Runtime, FakeWallet>, execution: &str) -> String {
    for n in 1..12 {
        s.tick(NOW + n).unwrap();
        if let Some(d) = s.lock().unwrap().journal.dispatch(execution).unwrap() {
            return d.resource;
        }
    }
    panic!("synthetic worker did not dispatch");
}
fn balance(s: &Service<Runtime, FakeWallet>, who: &str) -> Value {
    call(s, who, json!({"op":"account"})).unwrap()["result"]["balance"].clone()
}

#[test]
fn two_customers_and_read_only_principals_cannot_cross_authorities() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    assert!(matches!(
        call(&s, "bob", confirmation(&m)),
        Err(Error::Denied)
    ));
    let execution = call(&s, "alice", confirmation(&m)).unwrap()["result"]["execution"]
        .as_str()
        .unwrap()
        .to_owned();
    for op in ["execution", "progress", "cancel", "artifact", "receipt"] {
        let mut value = json!({"op":op,"execution":execution});
        if op == "progress" {
            value["after"] = json!(0);
        }
        if op == "artifact" {
            value["name"] = json!("patch");
        }
        assert!(matches!(call(&s, "bob", value), Err(Error::Denied)), "{op}");
    }
    for value in [
        json!({"op":"top_up","idempotency":"denied","amount_sats":10}),
        json!({"op":"offer","idempotency":"denied","task":request()}),
        confirmation(&m),
        json!({"op":"cancel","execution":execution}),
    ] {
        assert!(matches!(
            s.call(
                "reader",
                "reader",
                serde_json::from_value(value).unwrap(),
                NOW
            ),
            Err(Error::Denied)
        ));
    }
    assert_eq!(f.wallet.issued(), 0);
    assert_eq!(f.runtime.provider.create_calls(), 0);
    assert_eq!(f.runtime.owner.started(), 0);
    assert_eq!(balance(&s, "bob")["held_msat"], 0);
    // Explicitly granted observation may read the same account, without
    // manufacturing execution, disclosure, or spending from that read.
    assert!(
        s.call("reader", "reader", Request::Execution { execution }, NOW)
            .is_ok()
    );
}
#[test]
fn top_up_invoice_is_private_retry_safe_and_only_wallet_evidence_credits() {
    let f = Fixture::new();
    let s = f.open();
    let op = json!({"op":"top_up","idempotency":"topup","amount_sats":20});
    let p = call(&s, "alice", op.clone()).unwrap();
    assert_eq!(call(&s, "alice", op).unwrap(), p);
    assert_eq!(f.wallet.issued(), 1);
    let purchase = p["result"]["purchase"].as_str().unwrap();
    assert!(matches!(
        call(&s, "bob", json!({"op":"top_up_status","purchase":purchase})),
        Err(Error::Denied)
    ));
    assert!(matches!(
        call(
            &s,
            "alice",
            json!({"op":"top_up","idempotency":"topup","amount_sats":21})
        ),
        Err(Error::Lifecycle(retail_cloud::Error::Conflict(_)))
    ));
    assert_eq!(balance(&s, "alice")["credited_msat"], 1_000_000);
    f.wallet
        .pay_in_full(p["result"]["payment_hash"].as_str().unwrap());
    s.tick(NOW + 1).unwrap();
    s.tick(NOW + 2).unwrap();
    assert_eq!(balance(&s, "alice")["credited_msat"], 1_020_000);
    assert_eq!(balance(&s, "bob")["credited_msat"], 1_000_000);
}
#[test]
fn launch_gate_refuses_unqualified_funding_and_execution() {
    for kind in 0..3 {
        let mut f = Fixture::new();
        if kind == 0 {
            f.config.contract_confirmed = false;
        } else if kind == 1 {
            f.config.qualification = None;
        } else {
            f.config.qualification.as_mut().unwrap().mode = retail_qualify::qualify::Mode::Fake;
        }
        let s = f.open();
        let ad = call(&s, "alice", json!({"op":"capacity"})).unwrap();
        assert!(ad["result"]["paid_capacity"].is_null());
        assert!(matches!(
            call(
                &s,
                "alice",
                json!({"op":"offer","idempotency":"one","task":request()})
            ),
            Err(Error::Unavailable(_))
        ));
        assert!(matches!(
            call(
                &s,
                "alice",
                json!({"op":"top_up","idempotency":"one","amount_sats":10})
            ),
            Err(Error::Unavailable(_))
        ));
        assert_eq!(f.wallet.issued(), 0);
        assert_eq!(f.runtime.provider.create_calls(), 0);
    }
}
#[test]
fn exact_offer_approval_and_private_custody_are_required() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    for field in ["digest", "admission", "custody"] {
        let mut confirm = confirmation(&m);
        confirm[field] = json!(route_contract::Digest::of_bytes(b"changed"));
        assert!(matches!(
            call(&s, "alice", confirm),
            Err(Error::Conflict(_))
        ));
    }
    let mut confirm = confirmation(&m);
    confirm["credential"]["service_custody"] = json!(false);
    assert!(matches!(call(&s, "alice", confirm), Err(Error::Denied)));
    assert_eq!(s.lock().unwrap().journal.all_funded().unwrap().len(), 0);
    let original = call(&s, "alice", confirmation(&m)).unwrap();
    assert_eq!(call(&s, "alice", confirmation(&m)).unwrap(), original);
    let mut changed = confirmation(&m);
    changed["credential"]["key"] = json!("another-synthetic-key");
    assert!(matches!(
        call(&s, "alice", changed),
        Err(Error::Conflict(_))
    ));
    let store = s.lock().unwrap();
    assert_eq!(store.journal.all_funded().unwrap().len(), 1);
    let serialized = std::fs::read(f.config.state.join("transport.sqlite")).unwrap();
    assert!(
        !serialized
            .windows(CUSTOMER_KEY.len())
            .any(|w| w == CUSTOMER_KEY.as_bytes())
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(
            f.config
                .state
                .join("credentials")
                .join(m["offer"]["id"].as_str().unwrap())
        )
        .unwrap()
        .permissions()
        .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn lost_create_and_dispatch_replies_resume_same_identity_after_restart() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    f.runtime.provider.lose_next_ack();
    s.tick(NOW + 1).unwrap();
    assert_eq!(f.runtime.provider.create_calls(), 1);
    assert_eq!(f.runtime.provider.active().len(), 1);
    drop(s);
    let s = f.open();
    f.runtime.owner.lose_next_ack();
    let resource = dispatch(&s, &execution);
    assert_eq!(f.runtime.owner.started(), 1);
    drop(s);
    let s = f.open();
    s.tick(NOW + 30).unwrap();
    let repeat = accepted(&s, "alice", "one");
    assert_eq!(repeat, execution);
    assert_eq!(f.runtime.provider.create_calls(), 1);
    assert_eq!(f.runtime.owner.started(), 1);
    assert_eq!(f.runtime.provider.active(), vec![resource]);
    assert_eq!(s.lock().unwrap().journal.all_funded().unwrap().len(), 1);
}
#[test]
fn revoked_identity_stops_and_cleans_without_customer_access() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    let resource = dispatch(&s, &execution);
    s.lock()
        .unwrap()
        .ledger
        .revoke_principal("cli:alice", NOW + 10)
        .unwrap();
    assert!(matches!(
        call(&s, "alice", json!({"op":"account"})),
        Err(Error::Denied)
    ));
    s.tick(NOW + 100).unwrap();
    s.tick(NOW + 101).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert!(f.runtime.sandbox.files(&resource).is_empty());
    assert_eq!(f.runtime.stop_calls.load(Ordering::SeqCst), 1);
    assert!(
        std::fs::read_dir(f.config.state.join("credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(
        s.lock()
            .unwrap()
            .ledger
            .compute_balance("alice")
            .unwrap()
            .held_msat,
        0
    );
}
#[test]
fn rotation_invalidates_previous_execution_and_disclosure_epoch() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    s.lock()
        .unwrap()
        .ledger
        .rotate_principal("cli:alice", &credential_digest("new-alice"))
        .unwrap();
    assert!(matches!(
        call(&s, "alice", json!({"op":"account"})),
        Err(Error::Denied)
    ));
    s.tick(NOW + 100).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert_eq!(f.runtime.owner.started(), 1);
}
#[test]
fn lost_stop_reply_is_reconciled_and_does_not_repeat_stop() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    f.runtime.lost_stop.store(true, Ordering::SeqCst);
    call(&s, "alice", json!({"op":"cancel","execution":execution})).unwrap();
    s.tick(NOW + 100).unwrap();
    drop(s);
    let s = f.open();
    s.tick(NOW + 101).unwrap();
    assert_eq!(f.runtime.stop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(balance(&s, "alice")["held_msat"], 0);
    assert!(f.runtime.provider.active().is_empty());
}
#[test]
fn unknown_final_usage_remains_held_until_evidenced_reconciliation() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    let resource = dispatch(&s, &execution);
    f.runtime.provider.set_usage(&resource, 17);
    f.runtime.provider.set_usage_unreadable(true);
    call(&s, "alice", json!({"op":"cancel","execution":execution})).unwrap();
    s.tick(NOW + 100).unwrap();
    let held = balance(&s, "alice")["held_msat"].as_i64().unwrap();
    assert!(held > 0);
    assert!(f.runtime.provider.active().is_empty());
    drop(s);
    let s = f.open();
    s.tick(NOW + 101).unwrap();
    assert_eq!(balance(&s, "alice")["held_msat"], held);
    f.runtime.provider.set_usage_unreadable(false);
    s.tick(NOW + 102).unwrap();
    s.tick(NOW + 103).unwrap();
    let b = balance(&s, "alice");
    assert_eq!(b["held_msat"], 0);
    assert_eq!(
        b["credited_msat"].as_i64().unwrap(),
        b["available_msat"].as_i64().unwrap() + b["settled_msat"].as_i64().unwrap()
    );
    let once = b.clone();
    s.tick(NOW + 104).unwrap();
    assert_eq!(balance(&s, "alice"), once);
}
#[test]
fn provider_loss_after_dispatch_never_replaces_the_admitted_sandbox() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    let resource = dispatch(&s, &execution);
    f.runtime.provider.lose(&resource);
    s.tick(NOW + 40).unwrap();
    drop(s);
    let s = f.open();
    s.tick(NOW + 41).unwrap();
    assert_eq!(f.runtime.provider.create_calls(), 1);
    assert_eq!(f.runtime.owner.started(), 1);
    let receipt = call(&s, "alice", json!({"op":"receipt","execution":execution})).unwrap();
    assert_eq!(
        receipt["result"]["settlement"]["ending"],
        "provider_lost_after_executor"
    );
}
#[test]
fn client_disconnection_does_not_stop_worker_deadline_cleanup() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    s.tick(NOW + 2000).unwrap();
    s.tick(NOW + 2001).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert_eq!(balance(&s, "alice")["held_msat"], 0);
    let result = call(
        &s,
        "alice",
        json!({"op":"artifact","execution":execution,"name":"patch"}),
    )
    .unwrap();
    assert_eq!(result["result"]["text"], "synthetic patch\n");
    assert!(
        s.call(
            "cli:alice",
            "alice",
            Request::Artifact {
                execution,
                name: "patch".into()
            },
            NOW + 2001 + 31 * 86400
        )
        .is_err()
    );
}
#[test]
fn private_state_rejects_symlinks_and_multiple_workers() {
    let f = Fixture::new();
    let s = f.open();
    assert!(Service::open(f.config.clone(), f.runtime.clone(), f.wallet.clone()).is_err());
    drop(s);
    let link = f.config.state.join("credentials").join("link");
    std::os::unix::fs::symlink(&f.config.ledger, &link).unwrap();
    assert!(store::vault_read(&f.config.state, "link").is_err());
}
#[tokio::test]
async fn actual_http_adapter_bounds_authentication_origin_and_private_responses() {
    let f = Fixture::new();
    let s = Arc::new(f.open());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let host = tokio::spawn(axum::serve(listener, http::router(s.clone())).into_future());
    let client = reqwest::Client::new();
    let url = format!("http://{address}/v1/retail");
    let send = |who: &str, body: Value| {
        client
            .post(&url)
            .header("x-retail-principal", format!("cli:{who}"))
            .bearer_auth(who)
            .json(&body)
    };
    assert_eq!(
        client
            .post(&url)
            .json(&json!({"op":"account"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        send("alice", json!({"op":"account"}))
            .header("origin", "https://untrusted.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = send("alice", json!({"op":"account"})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        response.json::<Value>().await.unwrap()["result"]["account"],
        "alice"
    );
    assert_eq!(
        send("alice", json!({"op":"account","extra":CUSTOMER_KEY}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .post(&url)
            .body("x".repeat(types::BODY_MAX + 1))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    let m = send(
        "alice",
        json!({"op":"offer","idempotency":"http","task":request()}),
    )
    .send()
    .await
    .unwrap()
    .json::<Value>()
    .await
    .unwrap()["result"]
        .clone();
    assert_eq!(
        send("bob", confirmation(&m)).send().await.unwrap().status(),
        403
    );
    let result = send("alice", confirmation(&m))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let execution = result["result"]["execution"].as_str().unwrap();
    dispatch(&s, execution);
    f.runtime.owner.emit(
        &f.runtime.provider.active()[0],
        &retail_cloud::dispatch::task_id(execution),
        &format!("synthetic progress {CUSTOMER_KEY}"),
    );
    let result = send(
        "alice",
        json!({"op":"progress","execution":execution,"after":0}),
    )
    .send()
    .await
    .unwrap()
    .text()
    .await
    .unwrap();
    assert!(!result.contains(CUSTOMER_KEY));
    assert!(result.contains("[redacted]"));
    host.abort();
}
#[test]
fn resident_worker_runs_after_all_customer_calls_end_and_restarts() {
    let f = Fixture::new();
    let s = Arc::new(f.open());
    let made = s
        .call(
            "cli:alice",
            "alice",
            serde_json::from_value(json!({"op":"offer","idempotency":"resident","task":request()}))
                .unwrap(),
            http::now(),
        )
        .unwrap()["result"]
        .clone();
    let execution = s
        .call(
            "cli:alice",
            "alice",
            serde_json::from_value(confirmation(&made)).unwrap(),
            http::now(),
        )
        .unwrap()["result"]["execution"]
        .as_str()
        .unwrap()
        .to_owned();
    // Real thread lifetime, private persisted journals, and fresh service
    // ownership on restart; all collaborators remain synthetic.
    let worker = s.spawn_worker(Duration::from_secs(1));
    for _ in 0..70 {
        if f.runtime.owner.started() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(f.runtime.owner.started(), 1);
    drop(worker);
    drop(s);
    let s = Arc::new(f.open());
    f.runtime.clock.store(http::now(), Ordering::SeqCst);
    s.call(
        "cli:alice",
        "alice",
        serde_json::from_value(json!({"op":"cancel","execution":execution})).unwrap(),
        http::now(),
    )
    .unwrap();
    let worker = s.spawn_worker(Duration::from_secs(1));
    for _ in 0..30 {
        if f.runtime.provider.active().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(f.runtime.provider.active().is_empty());
    drop(worker);
}

#[test]
fn disclosure_withdrawal_on_restart_preserves_cleanup_without_new_upload() {
    let mut f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    drop(s);
    f.config
        .grants
        .iter_mut()
        .find(|g| g.principal == "cli:alice")
        .unwrap()
        .disclose = false;
    let s = f.open();
    s.tick(NOW + 100).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert_eq!(f.runtime.owner.started(), 1);
    assert_eq!(balance(&s, "alice")["held_msat"], 0);
    assert!(matches!(
        call(
            &s,
            "alice",
            json!({"op":"offer","idempotency":"new","task":request()})
        ),
        Err(Error::Denied)
    ));
}
#[test]
fn completed_task_retains_exact_candidate_checks_and_seals_one_charge() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    let resource = dispatch(&s, &execution);
    let patch = retail_cloud::sha256_hex(b"synthetic patch\n");
    f.runtime.owner.set_status(
        &resource,
        &retail_cloud::dispatch::task_id(&execution),
        TaskStatus::Ended {
            end: retail_cloud::dispatch::ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![retail_cloud::dispatch::CheckRun {
                command: "cargo test -p parser".into(),
                candidate: patch,
                exit_status: 0,
            }],
        },
    );
    f.runtime.provider.set_usage(&resource, 17);
    s.tick(NOW + 40).unwrap();
    let receipt = call(&s, "alice", json!({"op":"receipt","execution":execution})).unwrap();
    assert_eq!(receipt["result"]["settlement"]["checks"], "verified");
    assert_eq!(receipt["result"]["settlement"]["ending"], "executor_ended");
    let balance = balance(&s, "alice");
    s.tick(NOW + 41).unwrap();
    assert_eq!(s.tick(NOW + 42).unwrap().visited, 0);
    assert_eq!(crate::tests::balance(&s, "alice"), balance);
    assert_eq!(accepted(&s, "alice", "one"), execution);
}
#[test]
fn offer_expiry_causes_no_disclosure_or_execution() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    assert!(matches!(
        s.call(
            "cli:alice",
            "alice",
            serde_json::from_value(confirmation(&m)).unwrap(),
            NOW + 601
        ),
        Err(Error::Conflict(_))
    ));
    assert!(
        std::fs::read_dir(f.config.state.join("credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(s.lock().unwrap().journal.all_funded().unwrap().len(), 0);
    assert_eq!(f.runtime.provider.create_calls(), 0);
}
#[test]
fn accepted_offer_capacity_cannot_overbook_without_started_resources() {
    let f = Fixture::new();
    let s = f.open();
    for n in 0..4 {
        accepted(&s, "alice", &format!("offer-{n}"));
    }
    assert!(matches!(
        call(
            &s,
            "bob",
            json!({"op":"offer","idempotency":"five","task":request()})
        ),
        Err(Error::Unavailable(_))
    ));
    assert_eq!(f.runtime.provider.create_calls(), 0);
    let response = call(&s, "alice", json!({"op":"executions","after":null})).unwrap();
    assert_eq!(
        response["result"]["executions"].as_array().unwrap().len(),
        4
    );
    assert_eq!(
        call(&s, "bob", json!({"op":"executions","after":null})).unwrap()["result"]["executions"],
        json!([])
    );
}

#[test]
fn insufficient_balance_never_accepts_custody_or_provisions() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    s.lock()
        .unwrap()
        .ledger
        .reserve(&pay_ledger::compute::HoldRequest {
            id: "other-funded-request".into(),
            account: "alice".into(),
            quote: "other-pinned-quote".into(),
            execution: "other-execution".into(),
            terms: "other-admission".into(),
            amount_msat: 1_000_000,
            at: NOW,
        })
        .unwrap();
    assert!(matches!(
        call(&s, "alice", confirmation(&m)),
        Err(Error::Insufficient)
    ));
    assert_eq!(s.lock().unwrap().journal.all_funded().unwrap().len(), 0);
    assert!(
        std::fs::read_dir(f.config.state.join("credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(f.runtime.provider.create_calls(), 0);
}
#[test]
fn bounded_worker_meter_keeps_late_final_evidence_reconcilable() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    let resource = dispatch(&s, &execution);
    let mut store = s.lock().unwrap();
    for sequence in 1..=4000 {
        store
            .journal
            .record_usage(
                &execution,
                &retail_cloud::meter::Reading {
                    event: format!("fixture:{sequence}"),
                    sequence,
                    resource: resource.clone(),
                    at: NOW + 100 + sequence as i64,
                    seconds: Some(sequence as u64),
                    stopped: false,
                    model: None,
                },
            )
            .unwrap();
    }
    f.runtime.provider.set_usage(&resource, 4001);
    let usage = retail_cloud::meter::poll_bounded(
        &mut store.journal,
        &*f.runtime,
        &execution,
        "fixture:bounded",
        NOW + 4101,
    )
    .unwrap();
    assert_eq!(usage.events.len(), 4000);
    drop(store);
    f.runtime.provider.set_usage_unreadable(true);
    call(&s, "alice", json!({"op":"cancel","execution":execution})).unwrap();
    s.tick(NOW + 4102).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert!(balance(&s, "alice")["held_msat"].as_i64().unwrap() > 0);
    f.runtime.provider.set_usage_unreadable(false);
    s.tick(NOW + 4103).unwrap();
    assert_eq!(balance(&s, "alice")["held_msat"], 0);
    assert_eq!(
        s.lock()
            .unwrap()
            .journal
            .usage(&execution)
            .unwrap()
            .unwrap()
            .events
            .len(),
        4001
    );
}

#[test]
fn secret_embedded_in_artifact_is_refused_before_retention_and_cleanup_continues() {
    let f = Fixture::new();
    let s = f.open();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    *f.runtime.artifact_bytes.lock().unwrap() =
        Some(format!("prefix{CUSTOMER_KEY}suffix").into_bytes());
    call(&s, "alice", json!({"op":"cancel","execution":execution})).unwrap();
    s.tick(NOW + 100).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    let receipt = call(&s, "alice", json!({"op":"receipt","execution":execution})).unwrap();
    assert_eq!(receipt["result"]["retention"]["complete"], false);
    assert_eq!(receipt["result"]["retention"]["discovery_complete"], true);
    assert!(
        call(
            &s,
            "alice",
            json!({"op":"artifact","execution":execution,"name":"patch"})
        )
        .is_err()
    );
    let bytes = std::fs::read(f.config.state.join("lifecycle.sqlite")).unwrap();
    assert!(
        !bytes
            .windows(CUSTOMER_KEY.len())
            .any(|w| w == CUSTOMER_KEY.as_bytes())
    );
    assert_eq!(balance(&s, "alice")["held_msat"], 0);
}
#[test]
fn durable_confirmation_intent_resumes_when_initial_reply_or_append_was_lost() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    let offer_id = m["offer"]["id"].as_str().unwrap();
    // Model the crash boundary after the private synced custody file and
    // confirmation commit, before the separate lifecycle/hold append.
    {
        let store = s.lock().unwrap();
        let (offer, principal, generation, _) = store.offer(offer_id).unwrap().unwrap();
        store::vault_write(&f.config.state, offer_id, CUSTOMER_KEY).unwrap();
        store
            .confirmation(
                offer_id,
                &types::Confirmation {
                    principal,
                    generation,
                    admission: offer.admission.digest(),
                    custody: custody_digest(&offer),
                    key_digest: retail_cloud::sha256_hex(CUSTOMER_KEY.as_bytes()),
                    at: NOW as u64,
                },
            )
            .unwrap();
        assert!(store.journal.all_funded().unwrap().is_empty());
    }
    drop(s);
    let s = f.open();
    s.tick(NOW + 1).unwrap();
    let execution = accepted(&s, "alice", "one");
    dispatch(&s, &execution);
    assert_eq!(s.lock().unwrap().journal.all_funded().unwrap().len(), 1);
    assert_eq!(f.runtime.provider.create_calls(), 1);
    assert_eq!(f.runtime.owner.started(), 1);
}

#[test]
fn custom_templates_cannot_inherit_the_supported_retail_class() {
    let mut f = Fixture::new();
    f.config.template = "operator-environment-template".into();
    assert!(Service::open(f.config.clone(), f.runtime.clone(), f.wallet.clone()).is_err());
    assert_eq!(f.runtime.provider.create_calls(), 0);
}

#[tokio::test]
async fn partial_body_connections_cannot_grow_the_transport_queue_without_bound() {
    use std::io::Write;
    let f = Fixture::new();
    let s = Arc::new(f.open());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let host = tokio::spawn(axum::serve(listener, http::router(s.clone())).into_future());
    let mut streams = Vec::new();
    for _ in 0..32 {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        write!(
            stream,
            "POST /v1/retail HTTP/1.1\r\nHost: {address}\r\nContent-Length: 100\r\n\r\n"
        )
        .unwrap();
        streams.push(stream);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/retail"))
        .header("x-retail-principal", "cli:alice")
        .bearer_auth("alice")
        .json(&json!({"op":"account"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
    assert_eq!(response.json::<Value>().await.unwrap()["error"], "busy");
    assert_eq!(f.wallet.issued(), 0);
    assert_eq!(f.runtime.provider.create_calls(), 0);
    drop(streams);
    host.abort();
}

#[test]
fn revocation_during_provider_reconciliation_prevents_later_material_delivery() {
    let f = Fixture::new();
    let s = f.open();
    accepted(&s, "alice", "one");
    s.tick(NOW + 1).unwrap();
    *f.runtime.revoke_path.lock().unwrap() = Some(f.config.ledger.clone());
    s.tick(NOW + 2).unwrap();
    assert!(f.runtime.provider.active().is_empty());
    assert!(f.runtime.sandbox.commands().is_empty());
    assert_eq!(f.runtime.owner.started(), 0);
    assert_eq!(
        s.lock()
            .unwrap()
            .ledger
            .compute_balance("alice")
            .unwrap()
            .held_msat,
        0
    );
}

#[test]
fn revocation_after_confirmation_intent_before_hold_creates_no_spend_or_stranded_custody() {
    let f = Fixture::new();
    let s = f.open();
    let m = made(&s, "alice", "one");
    let id = m["offer"]["id"].as_str().unwrap();
    {
        let mut store = s.lock().unwrap();
        let (offer, principal, generation, _) = store.offer(id).unwrap().unwrap();
        store::vault_write(&f.config.state, id, CUSTOMER_KEY).unwrap();
        store
            .confirmation(
                id,
                &types::Confirmation {
                    principal,
                    generation,
                    admission: offer.admission.digest(),
                    custody: custody_digest(&offer),
                    key_digest: retail_cloud::sha256_hex(CUSTOMER_KEY.as_bytes()),
                    at: NOW as u64,
                },
            )
            .unwrap();
        store.ledger.revoke_principal("cli:alice", NOW + 1).unwrap();
    }
    drop(s);
    let s = f.open();
    let report = s.tick(NOW + 2).unwrap();
    assert!(report.failed.is_empty());
    assert_eq!(report.pending, 0);
    assert_eq!(f.runtime.provider.create_calls(), 0);
    assert_eq!(f.runtime.owner.started(), 0);
    assert!(
        std::fs::read_dir(f.config.state.join("credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    let b = s.lock().unwrap().ledger.compute_balance("alice").unwrap();
    assert_eq!(b.available_msat, 1_000_000);
    assert_eq!(b.held_msat, 0);
    assert_eq!(b.settled_msat, 0);
    assert_eq!(s.tick(NOW + 3).unwrap().visited, 0);
}
