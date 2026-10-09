//! Synthetic native-counter fixtures over the real admission, ledgers, and
//! sealed receipt path. No model weights, owner state, or real funds are used.

#[path = "decision_offer_parts/budgets.rs"]
mod budgets;
#[path = "decision_offer_parts/reports.rs"]
mod reports;
#[path = "decision_offer_parts/shared.rs"]
mod shared;

use axum::{
    Json,
    extract::State,
    routing::{get, post},
};
use gateway::{
    config::Config,
    decision_offer::{SCHEMA, SelectedOffer},
    money::Priced,
    serve::{self, ServeState},
};
use receipts::decision_metering::Metering;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tenancy::money::{CreditKind, Ledger, Mutation, Operation, Price, Rate, Resource};
use tenancy::{Binding, Capacity, Expected, Lane, Manifest, Registry, Tenant, keys};
use tokio::{
    sync::{Notify, oneshot},
    task::JoinHandle,
};

const DOOR: &str = "buyer-kev";
const HOLD: u64 = 448;
fn identity() -> Expected {
    Expected {
        model: "kev-0.6b".into(),
        adapter: None,
        artifact_signature: format!("sha256:{}", "b".repeat(64)),
        execution: [
            ("backend", "cpu"),
            ("dtype", "f32"),
            ("head_dtype", "f32"),
            ("attention", "eager-block-causal-v1"),
            ("bucket_size", "0"),
            ("lora_merge", "fp32-before-cast-v1"),
            ("option_isolation", "false"),
            ("max_state", "8192"),
            ("max_branch", "8192"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
    }
}
fn priced() -> Priced {
    Priced {
        offer: Some(SelectedOffer {
            schema: SCHEMA.into(),
            id: "kev-decision-access".into(),
            version: "synthetic-offer-v1".into(),
            identity: identity(),
            requests_per_minute: 60,
        }),
        price: Price {
            version: "synthetic-price-v1".into(),
            currency: "USD".into(),
            model: "kev-0.6b".into(),
            capacity: "dedicated".into(),
            policy: gateway::money::POLICY.into(),
            rates: [(
                Resource::InputTokens,
                Rate {
                    millionths: 7,
                    per_units: 2,
                },
            )]
            .into(),
        },
        maximum_usage: [(Resource::InputTokens, 128)].into(),
    }
}
struct Backend {
    card: Mutex<Value>,
    reply: Mutex<Value>,
    requests: Mutex<Vec<Value>>,
    forwards: AtomicUsize,
    block: AtomicBool,
    card_block: AtomicBool,
    card_entered: Notify,
    card_resume: Notify,
    entered: Notify,
    resume: Notify,
}
async fn models(State(b): State<Arc<Backend>>) -> Json<Value> {
    if b.card_block.load(Ordering::SeqCst) {
        b.card_entered.notify_one();
        b.card_resume.notified().await;
    }
    Json(json!({"models":[b.card.lock().unwrap().clone()]}))
}
async fn evaluate(State(b): State<Arc<Backend>>, Json(request): Json<Value>) -> Json<Value> {
    b.requests.lock().unwrap().push(request);
    b.forwards.fetch_add(1, Ordering::SeqCst);
    b.entered.notify_one();
    if b.block.load(Ordering::SeqCst) {
        b.resume.notified().await;
    }
    Json(b.reply.lock().unwrap().clone())
}
struct Host {
    dir: tempfile::TempDir,
    backend: Arc<Backend>,
    backend_server: JoinHandle<()>,
    config: Config,
    token: String,
    workspace: String,
    foreign: String,
    address: String,
    state: Option<Arc<ServeState>>,
    shutdown: Option<oneshot::Sender<()>>,
    server: Option<JoinHandle<()>>,
}
impl Drop for Host {
    fn drop(&mut self) {
        self.backend_server.abort();
        if let Some(server) = &self.server {
            server.abort();
        }
    }
}
impl Host {
    async fn new(credit: u64, tune: impl FnOnce(&mut Manifest, &mut Option<Priced>)) -> Self {
        let id = identity();
        let backend = Arc::new(Backend {
            card: Mutex::new(
                json!({"id":id.model,"artifact_identity":{"digest":id.artifact_signature},"execution":id.execution,"batching":{"kind":"caller-loop"},"limits":{"context_tokens":128,"concurrent_calls":1},"metering":Metering::kev_packed_input(128)}),
            ),
            reply: Mutex::new(
                json!({"model":"kev-0.6b","answers":{"q":{"type":"noul","noul":0.75}},"usage":{"input_tokens":3,"output_tokens":99}}),
            ),
            requests: Mutex::new(vec![]),
            forwards: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            card_block: AtomicBool::new(false),
            card_entered: Notify::new(),
            card_resume: Notify::new(),
            entered: Notify::new(),
            resume: Notify::new(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = axum::Router::new()
            .route("/v1/models", get(models))
            .route("/v1/systemone", post(evaluate))
            .with_state(backend.clone());
        let backend_server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut manifest = Manifest {
            v: tenancy::SCHEMA.into(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants: [(
                "buyer".into(),
                Tenant {
                    credential: "key-ref:buyer".into(),
                    principals: vec![],
                    doors: [(
                        DOOR.into(),
                        Binding {
                            lane: Lane::Dedicated,
                            artifact: identity(),
                            capacity: Some(Capacity {
                                concurrency: Some(1),
                                requests_per_minute: Some(60),
                            }),
                            promotion: None,
                            scope: vec![],
                        },
                    )]
                    .into(),
                    quota: None,
                },
            )]
            .into(),
            digest: String::new(),
        };
        let mut price = Some(priced());
        tune(&mut manifest, &mut price);
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::install(dir.path(), manifest).unwrap();
        let issued = keys::issue(dir.path(), registry.manifest(), "buyer").unwrap();
        let accounts = tenancy::Accounts::install(dir.path()).unwrap();
        let account = accounts
            .create_account("fixture buyer", &[format!("key:{}", issued.key.id)])
            .unwrap();
        let workspace = accounts
            .create_workspace(
                &account.id,
                "fixture",
                tenancy::WorkspaceKind::Personal,
                "buyer",
                None,
            )
            .unwrap()
            .id;
        let other = accounts.create_account("other fixture buyer", &[]).unwrap();
        let foreign = accounts
            .create_workspace(
                &other.id,
                "other",
                tenancy::WorkspaceKind::Personal,
                "buyer",
                None,
            )
            .unwrap()
            .id;
        let ledger_path = dir.path().join("money.jsonl");
        {
            let mut ledger = Ledger::open(&ledger_path).unwrap();
            ledger
                .apply(Mutation {
                    workspace: workspace.clone(),
                    source: "synthetic:create".into(),
                    audit: "isolated fixture".into(),
                    operation: Operation::Create {
                        currency: "USD".into(),
                        spend_limit: 10_000,
                        topups_allowed: false,
                    },
                })
                .unwrap();
            if credit > 0 {
                ledger
                    .apply(Mutation {
                        workspace: workspace.clone(),
                        source: "synthetic:grant".into(),
                        audit: "not purchased funds".into(),
                        operation: Operation::Credit {
                            amount: credit,
                            credit_kind: CreditKind::Grant,
                        },
                    })
                    .unwrap();
            }
        }
        let prices: BTreeMap<String, Priced> =
            price.map(|p| [(DOOR.into(), p)].into()).unwrap_or_default();
        let config:Config=serde_json::from_value(json!({"v":gateway::config::SCHEMA,"listen":"127.0.0.1:0","registry":dir.path(),"require_workspace_membership":true,"accounts":{},"money":{"ledger":ledger_path,"doors":prices},"doors":{DOOR:{"endpoint":endpoint}},"forward_timeout_ms":5_000})).unwrap();
        let state = ServeState::open(config.clone()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (shutdown, receive) = oneshot::channel();
        let router = serve::router(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = receive.await;
                })
                .await
                .unwrap();
        });
        Self {
            dir,
            backend,
            backend_server,
            config,
            token: issued.token,
            workspace,
            foreign,
            address,
            state: Some(state),
            shutdown: Some(shutdown),
            server: Some(server),
        }
    }
    async fn call(&self, id: &str) -> reqwest::Response {
        send(&self.address, &self.token, &self.workspace, id).await
    }
    async fn balance(&self) -> Value {
        reqwest::Client::new()
            .get(format!("{}/v1/balance", self.address))
            .bearer_auth(&self.token)
            .header("x-workspace-id", &self.workspace)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    fn receipts(&self) -> Vec<receipts::ExecutionReceipt> {
        std::fs::read_to_string(self.dir.path().join("receipts.jsonl"))
            .unwrap()
            .lines()
            .map(|line| receipts::ExecutionReceipt::parse(line).unwrap())
            .collect()
    }
    async fn stop(&mut self) {
        let _ = self.shutdown.take().unwrap().send(());
        self.server.take().unwrap().await.unwrap();
        self.state.take();
    }
}
async fn send(address: &str, token: &str, workspace: &str, id: &str) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{address}/v1/systemone"))
        .bearer_auth(token)
        .header("x-workspace-id", workspace)
        .header("idempotency-key", id)
        .header("x-attempt", "1")
        .json(&request())
        .send()
        .await
        .unwrap()
}
fn request() -> Value {
    json!({"model":DOOR,"state":"Synthetic fixture state.","questions":{"q":{"type":"noul","instructions":"Does the fixture mention routing?"}}})
}

#[tokio::test]
async fn selected_offer_exact_terms_charge_receipt_and_retry_use_one_existing_ledger() {
    let host = Host::new(1_000, |_, _| {}).await;
    let cards: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", host.address))
        .bearer_auth(&host.token)
        .header("x-workspace-id", &host.workspace)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let terms = &cards["models"][0]["decision_offer"];
    assert_eq!(terms["schema"], gateway::decision_offer::TERMS_SCHEMA);
    assert_eq!(terms["status"], "configured");
    assert_eq!(terms["maximum_charge"], HOLD);
    assert_eq!(terms["currency_scale"], 1_000_000);
    assert_eq!(
        terms["price"]["rates"]["input-tokens"],
        json!({"millionths":7,"per_units":2})
    );
    assert_eq!(terms["account_currency"], "USD");
    assert_eq!(terms["currency_compatible"], true);
    assert_eq!(terms["funding_admission"]["status"], "ceiling-affordable");
    assert_eq!(terms["funding_admission"]["authorizes_dispatch"], false);
    assert_eq!(terms["purchase_context"]["price"]["maximum_charge"], HOLD);
    assert_eq!(terms["payer_workspace"], host.workspace);
    assert_eq!(terms["qualification"]["state"], "unknown");
    assert_eq!(terms["expense"]["provider"]["status"], "unknown");
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 0);
    let forbidden = reqwest::Client::new()
        .get(format!("{}/v1/models", host.address))
        .bearer_auth(&host.token)
        .header("x-workspace-id", &host.foreign)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);
    let response = host.call("first").await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["x-settlement"], "settled");
    let _: Value = response.json().await.unwrap();
    let balance = host.balance().await;
    assert_eq!(balance["balance"]["settled"], 11);
    assert_eq!(balance["balance"]["reserved"], 0);
    assert_eq!(balance["balance"]["available"], 989);
    assert_eq!(
        host.backend.requests.lock().unwrap()[0]["model"],
        "kev-0.6b"
    );
    assert_eq!(
        host.backend.requests.lock().unwrap()[0]["state"],
        "Synthetic fixture state."
    );
    let receipt = host.receipts().pop().unwrap();
    assert_eq!(
        receipt.request_digest,
        receipts::execution::digest_request(&request())
    );
    assert_ne!(
        receipt.request_digest,
        receipts::execution::digest_request(&host.backend.requests.lock().unwrap()[0])
    );
    assert_eq!(receipt.outcome, receipts::Outcome::Answered);
    assert_eq!(
        receipt.served.artifact_signature,
        identity().artifact_signature
    );
    assert_eq!(receipt.usage, Some("first#1".into()));
    let retry = host.call("first").await;
    // The decision route uses NIP-DEC's named status for this refusal.
    assert_eq!(retry.status(), 400);
    let retry: Value = retry.json().await.unwrap();
    assert_eq!(retry["error"]["code"], "idempotency_conflict");
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn selected_offer_identity_counter_units_bounds_price_and_funds_refuse_dispatch() {
    for case in [
        "identity",
        "units",
        "overlap",
        "backend-bound",
        "backend-lower-bound",
        "missing-metering",
        "missing-price",
        "funds",
        "capacity",
    ] {
        let host = Host::new(if case == "funds" { 0 } else { 1_000 }, |m, p| {
            if case == "missing-price" {
                *p = None;
            }
            if case == "capacity" {
                m.tenants
                    .get_mut("buyer")
                    .unwrap()
                    .doors
                    .get_mut(DOOR)
                    .unwrap()
                    .capacity
                    .as_mut()
                    .unwrap()
                    .concurrency = Some(2);
            }
        })
        .await;
        {
            let mut card = host.backend.card.lock().unwrap();
            match case {
                "identity" => {
                    card["artifact_identity"]["digest"] =
                        json!(format!("sha256:{}", "c".repeat(64)))
                }
                "units" => {
                    card["metering"]["counters"]["input_tokens"]["unit"] = json!("milliseconds")
                }
                "overlap" => {
                    card["metering"]["counters"]["input_tokens"]["overlaps"] =
                        json!(["cached_input_tokens"])
                }
                "backend-bound" => {
                    card["metering"] = json!(Metering::kev_packed_input(129));
                    card["limits"]["context_tokens"] = json!(129);
                }
                "backend-lower-bound" => {
                    card["metering"] = json!(Metering::kev_packed_input(64));
                    card["limits"]["context_tokens"] = json!(64);
                }
                "missing-metering" => {
                    card.as_object_mut().unwrap().remove("metering");
                }
                _ => {}
            }
        }
        let response = host.call(case).await;
        assert!(
            response.status().is_client_error() || response.status().is_server_error(),
            "{case}"
        );
        let body: Value = response.json().await.unwrap();
        assert_eq!(
            body["error"]["code"],
            match case {
                "missing-price" => "unpriced",
                "funds" => "insufficient_funds",
                "capacity" => "price_invalid",
                _ => "identity_mismatch",
            },
            "{case}"
        );
        assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 0, "{case}");
        let balance = host.balance().await;
        assert_eq!(balance["balance"]["reserved"], 0, "{case}");
        assert_eq!(balance["balance"]["settled"], 0, "{case}");
    }
}

#[tokio::test]
async fn selected_offer_missing_malformed_overbound_or_changed_model_keeps_full_unknown_hold() {
    for case in ["missing", "malformed", "over-bound", "model"] {
        let mut host = Host::new(1_000, |_, _| {}).await;
        {
            let mut reply = host.backend.reply.lock().unwrap();
            match case {
                "missing" => {
                    reply.as_object_mut().unwrap().remove("usage");
                }
                "malformed" => reply["usage"]["input_tokens"] = json!(1.5),
                "over-bound" => reply["usage"]["input_tokens"] = json!(129),
                "model" => reply["model"] = json!("different-model"),
                _ => unreachable!(),
            }
        }
        let response = host.call(case).await;
        assert_eq!(response.headers()["x-settlement"], "outstanding", "{case}");
        let _: Value = response.json().await.unwrap();
        let balance = host.balance().await;
        assert_eq!(balance["balance"]["reserved"], HOLD);
        assert_eq!(balance["balance"]["settled"], 0);
        host.stop().await;
        let mut ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
        let attempt = format!("{case}#1");
        let hold = ledger.hold(&host.workspace, &attempt).unwrap();
        assert_eq!(hold.phase, tenancy::money::Phase::Unknown);
        assert_eq!(hold.provider_cost, None);
        assert_eq!(hold.hosting_cost, None);
        ledger
            .apply(Mutation {
                workspace: host.workspace.clone(),
                source: format!("synthetic:reconcile:{case}"),
                audit: "synthetic recovered counter evidence".into(),
                operation: Operation::Settle {
                    attempt,
                    usage: [(Resource::InputTokens, 3)].into(),
                    provider_cost: None,
                    hosting_cost: None,
                    receipt: "synthetic-reconciliation-evidence".into(),
                },
            })
            .unwrap();
        assert_eq!(ledger.balance(&host.workspace).unwrap().settled, 11);
    }
}

#[tokio::test]
async fn selected_offer_one_call_capacity_refuses_a_second_dispatch() {
    let host = Host::new(1_000, |_, _| {}).await;
    host.backend.block.store(true, Ordering::SeqCst);
    let (address, token, workspace) = (
        host.address.clone(),
        host.token.clone(),
        host.workspace.clone(),
    );
    let first = tokio::spawn(async move { send(&address, &token, &workspace, "one").await });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        host.backend.entered.notified(),
    )
    .await
    .unwrap();
    let second = host.call("two").await;
    assert_eq!(second.status(), 503);
    let body: Value = second.json().await.unwrap();
    assert_eq!(body["error"]["code"], "busy");
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    host.backend.resume.notify_one();
    let response = first.await.unwrap();
    assert_eq!(response.headers()["x-settlement"], "settled");
    let _: Value = response.json().await.unwrap();
}

#[test]
fn selected_offer_rejects_missing_or_incompatible_price_units_and_overflow() {
    let good = priced();
    let offer = good.offer.clone().unwrap();
    assert!(offer.check(&good).is_ok());
    for case in [
        "missing",
        "extra",
        "zero-denominator",
        "overflow",
        "identity",
        "execution",
    ] {
        let mut p = good.clone();
        match case {
            "missing" => p.price.rates.clear(),
            "extra" => {
                p.price.rates.insert(
                    Resource::CachedInputTokens,
                    Rate {
                        millionths: 1,
                        per_units: 1,
                    },
                );
                p.maximum_usage.insert(Resource::CachedInputTokens, 128);
            }
            "zero-denominator" => {
                p.price
                    .rates
                    .get_mut(&Resource::InputTokens)
                    .unwrap()
                    .per_units = 0
            }
            "overflow" => {
                p.price
                    .rates
                    .get_mut(&Resource::InputTokens)
                    .unwrap()
                    .millionths = u64::MAX;
                p.maximum_usage.insert(Resource::InputTokens, u64::MAX);
            }
            "identity" => p
                .offer
                .as_mut()
                .unwrap()
                .identity
                .artifact_signature
                .clear(),
            "execution" => {
                p.offer
                    .as_mut()
                    .unwrap()
                    .identity
                    .execution
                    .remove("max_state");
            }
            _ => unreachable!(),
        }
        assert!(p.offer.as_ref().unwrap().check(&p).is_err(), "{case}");
    }
}

#[tokio::test]
async fn selected_offer_price_version_reuses_retained_ledger_terms_after_restart() {
    let mut host = Host::new(1_000, |_, _| {}).await;
    let response = host.call("priced").await;
    let _: Value = response.json().await.unwrap();
    host.stop().await;
    let mut changed = host.config.clone();
    changed
        .money
        .as_mut()
        .unwrap()
        .doors
        .get_mut(DOOR)
        .unwrap()
        .price
        .rates
        .get_mut(&Resource::InputTokens)
        .unwrap()
        .millionths = 8;
    assert!(ServeState::open(changed.clone()).is_err());
    changed
        .money
        .as_mut()
        .unwrap()
        .doors
        .get_mut(DOOR)
        .unwrap()
        .price
        .version = "synthetic-price-v2".into();
    assert!(ServeState::open(changed).is_ok());
}

#[tokio::test]
async fn selected_offer_terms_preserve_balance_scope_and_missing_funds() {
    let host = Host::new(0, |_, _| {}).await;
    let registry = Registry::open(host.dir.path()).unwrap();
    let original = keys::authenticate(host.dir.path(), registry.manifest(), &host.token).unwrap();
    let scoped = keys::issue_scoped(
        host.dir.path(),
        registry.manifest(),
        "buyer",
        Some("fixture discovery"),
        Some(keys::Scopes {
            models: Some([DOOR.into()].into()),
            actions: Some(["models".into(), "accounts".into()].into()),
        }),
    )
    .unwrap();
    let accounts = tenancy::Accounts::open(host.dir.path()).unwrap();
    let principal = format!("key:{}", original.key_id);
    let account = accounts.account_of_principal(&principal).unwrap().unwrap();
    accounts
        .update_principals(&account, &[principal, format!("key:{}", scoped.key.id)])
        .unwrap();
    for (token, balance_permitted) in [(&host.token, true), (&scoped.token, false)] {
        let response = reqwest::Client::new()
            .get(format!("{}/v1/models", host.address))
            .bearer_auth(token)
            .header("x-workspace-id", &host.workspace)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        let terms = &body["models"][0]["decision_offer"];
        assert_eq!(terms["price"]["version"], "synthetic-price-v1");
        assert_eq!(terms["funding_admission"]["authorizes_dispatch"], false);
        if balance_permitted {
            assert_eq!(terms["spendable_for_this_price"], 0);
            assert_eq!(terms["funding_admission"]["status"], "insufficient-funds");
        } else {
            assert_eq!(terms["balance_visibility"], "out-of-scope");
            assert!(terms["account_currency"].is_null());
            assert!(terms["spendable_for_this_price"].is_null());
            assert!(terms["outstanding_reserved"].is_null());
            assert_eq!(terms["funding_admission"]["status"], "unknown");
            assert_eq!(terms["purchase_context"]["can_invoke"], false);
        }
    }
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 0);
}
