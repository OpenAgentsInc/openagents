//! The gateway's end-to-end contract: real HTTP both ways, a stubbed
//! backend publishing a model card, and a real registry, key store, and
//! ledger on disk. Each test stands up its own directory and listeners —
//! nothing shares state but the shape of the claims being checked.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};
use tenancy::quota::Ledger;
use tenancy::{Binding, Capacity, Expected, Lane, Manifest, Quota, Registry, Tenant, keys};

use gateway::config::{Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

/// A valid-looking artifact pin for test bindings.
fn artifact(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

/// The manifest the tests share: a shared door every caller may name,
/// `acme` with a dedicated door and a quota, and `globex` bound to
/// nothing but the shared set.
fn manifest(requests_per_day: Option<u64>) -> Manifest {
    let mut shared = BTreeMap::new();
    shared.insert(
        "shared-kev".to_string(),
        Binding {
            lane: Lane::Shared,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: artifact('a'),
                execution: BTreeMap::new(),
            },
            capacity: None,
            promotion: None,
        },
    );
    let mut acme_doors = BTreeMap::new();
    acme_doors.insert(
        "acme-kev".to_string(),
        Binding {
            lane: Lane::Dedicated,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: artifact('b'),
                execution: BTreeMap::new(),
            },
            capacity: Some(Capacity {
                concurrency: Some(2),
                requests_per_minute: None,
            }),
            promotion: None,
        },
    );
    let mut tenants = BTreeMap::new();
    tenants.insert(
        "acme".to_string(),
        Tenant {
            credential: "key-ref:acme".to_string(),
            principals: vec![],
            doors: acme_doors,
            quota: requests_per_day.map(|limit| Quota {
                requests_per_day: Some(limit),
                questions_per_day: None,
                input_bytes_per_day: None,
                concurrency: None,
                policy: Some(tenancy::quota::POLICY_V1.to_string()),
            }),
        },
    );
    tenants.insert(
        "globex".to_string(),
        Tenant {
            credential: "key-ref:globex".to_string(),
            principals: vec![],
            doors: BTreeMap::new(),
            quota: None,
        },
    );
    Manifest {
        v: tenancy::SCHEMA.to_string(),
        sequence: 0,
        supersedes: None,
        shared,
        tenants,
        digest: String::new(),
    }
}

/// A stub's per-request responder: the forwarded body to its answer.
type Responder = dyn Fn(&Value) -> (StatusCode, Value) + Send + Sync;

/// What a stub backend publishes and answers.
struct Backend {
    /// The card's model id.
    model: String,
    /// The digest `artifact_identity.digest` reports.
    digest: String,
    /// The status `POST /v1/systemone` answers with.
    answer_status: StatusCode,
    /// The body it answers with.
    answer_body: Value,
    /// How long the answer takes — the seam an in-flight retry needs.
    delay_ms: u64,
    /// How many forwards arrived — a refusal must not spend one.
    forwards: Arc<AtomicUsize>,
    /// Every forwarded body, so a test can read the questions the
    /// facade actually sent.
    bodies: Arc<Mutex<Vec<Value>>>,
    /// A per-request answer, when the test needs input-dependent
    /// replies — keyed off the request's state.
    respond: Option<Arc<Responder>>,
}

async fn backend_models(State(backend): State<Arc<Backend>>) -> Json<Value> {
    Json(json!({
        "models": [{
            "id": backend.model,
            "name": backend.model,
            "artifact_identity": {"digest": backend.digest},
            "execution": {},
        }],
    }))
}

async fn backend_systemone(State(backend): State<Arc<Backend>>, body: Bytes) -> Response {
    backend.forwards.fetch_add(1, Ordering::SeqCst);
    let parsed: Value = serde_json::from_slice(&body).unwrap_or_default();
    backend.bodies.lock().unwrap().push(parsed.clone());
    if backend.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backend.delay_ms)).await;
    }
    if let Some(respond) = &backend.respond {
        let (status, body) = respond(&parsed);
        return (status, Json(body)).into_response();
    }
    (backend.answer_status, Json(backend.answer_body.clone())).into_response()
}

/// Stand a stub backend up on a real port.
async fn backend(backend: Backend) -> (String, Arc<AtomicUsize>) {
    let forwards = backend.forwards.clone();
    let router = axum::Router::new()
        .route("/v1/models", get(backend_models))
        .route("/v1/systemone", post(backend_systemone))
        .with_state(Arc::new(backend));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{address}"), forwards)
}

/// The card an honest backend publishes for `kev-0.6b`.
fn honest(digest: String, body: Value) -> Backend {
    Backend {
        model: "kev-0.6b".to_string(),
        digest,
        answer_status: StatusCode::OK,
        answer_body: body,
        delay_ms: 0,
        forwards: Arc::new(AtomicUsize::new(0)),
        bodies: Arc::new(Mutex::new(Vec::new())),
        respond: None,
    }
}

/// A registry directory, a key per tenant, and the running gateway.
struct Deployment {
    /// The issued tokens, by tenant.
    tokens: BTreeMap<String, String>,
    /// The registry directory — receipts and ledger live inside.
    dir: tempfile::TempDir,
    /// The gateway's address.
    address: String,
    /// The state, kept alive so the ledger lock stays held.
    _state: Arc<ServeState>,
}

/// Stand the whole thing up: registry, keys, stub backend, gateway.
async fn deploy(manifest: Manifest, endpoints: BTreeMap<String, String>) -> Deployment {
    deploy_doors(
        manifest,
        endpoints
            .into_iter()
            .map(|(door, endpoint)| {
                (
                    door,
                    Door {
                        endpoint,
                        classify: None,
                    },
                )
            })
            .collect(),
    )
    .await
}

/// The same deployment, with each door's full declaration supplied —
/// classify bounds included.
async fn deploy_doors(manifest: Manifest, doors: BTreeMap<String, Door>) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_questions: 256,
        max_options: 4096,
        doors,
    };
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        dir,
        address,
        _state: state,
    }
}

/// The receipts file's sealed lines — each parses and self-verifies.
fn receipt_log(dir: &tempfile::TempDir) -> Vec<receipts::execution::ExecutionReceipt> {
    std::fs::read_to_string(dir.path().join("receipts.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| receipts::execution::ExecutionReceipt::parse(line).unwrap())
        .collect()
}

/// A decision request body.
fn call(model: &str) -> Value {
    json!({
        "model": model,
        "state": "A caller's private text.",
        "questions": {
            "q1": {"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"},
        },
    })
}

/// POST a call, with or without a key.
async fn send_call(
    deployment: &Deployment,
    body: &Value,
    token: Option<&str>,
) -> reqwest::Response {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/systemone", deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request.send().await.unwrap()
}

#[tokio::test]
async fn an_authorized_call_answers_and_leaves_a_receipt() {
    let (endpoint, forwards) =
        backend(honest(artifact('b'), json!({"answers": {"q1": 0.9}}))).await;
    let deployment = deploy(
        manifest(None),
        [("acme-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "answered");
    assert!(response.headers().get("x-receipt").is_some());
    assert!(response.headers().get("x-request-id").is_some());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["answers"]["q1"], 0.9);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);

    // The sealed receipt names who called, which registry admitted it,
    // and what artifact actually answered.
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Answered);
    assert!(receipt.tenant.is_some());
    assert_eq!(receipt.served.model, "kev-0.6b");
    assert_eq!(receipt.served.artifact_signature, artifact('b'));
    assert!(receipt.registry.is_some());
}

#[tokio::test]
async fn an_unbound_door_and_an_unknown_key_are_refused() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [
            ("shared-kev".to_string(), endpoint.clone()),
            ("acme-kev".to_string(), endpoint),
        ]
        .into_iter()
        .collect(),
    )
    .await;

    // globex holds no binding for acme's dedicated door.
    let response = send_call(
        &deployment,
        &call("acme-kev"),
        Some(&deployment.tokens["globex"]),
    )
    .await;
    assert_eq!(response.status(), 403);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "door_not_bound");

    // A made-up credential authenticates nothing.
    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some("oak_deadbeef.deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
    )
    .await;
    assert_eq!(response.status(), 401);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unauthenticated");

    // Neither refusal reached a backend.
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
    assert_eq!(receipt_log(&deployment.dir).len(), 2);
}

#[tokio::test]
async fn an_anonymous_call_reaches_only_the_shared_lane() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {"q1": 1}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(&deployment, &call("shared-kev"), None).await;
    assert_eq!(response.status(), 200);

    let response = send_call(&deployment, &call("acme-kev"), None).await;
    assert_eq!(response.status(), 403);
    assert_eq!(forwards.load(Ordering::SeqCst), 1);

    // The anonymous call's receipt carries no tenant.
    let first = &receipt_log(&deployment.dir)[0];
    assert_eq!(first.tenant, None);
}

#[tokio::test]
async fn quota_exhaustion_is_refused_and_counted() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(Some(1)),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let first = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(first.status(), 200);

    let second = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(second.status(), 429);
    assert!(second.headers().get("retry-after").is_some());
    let body: Value = second.json().await.unwrap();
    assert_eq!(body["error"]["code"], "quota_exhausted");
    assert_eq!(forwards.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_share_the_reservation_and_resolved_or_changed_pairs_refuse() {
    // A slow backend holds the first attempt open so a retry can arrive
    // while its reservation still stands.
    let (endpoint, forwards) = backend(Backend {
        delay_ms: 300,
        ..honest(artifact('a'), json!({"answers": {}}))
    })
    .await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;
    let client = reqwest::Client::new();
    let send = |body: Value| {
        client
            .post(format!("{}/v1/systemone", deployment.address))
            .bearer_auth(&deployment.tokens["acme"])
            .header("idempotency-key", "req-duplicate")
            .header("x-attempt", "1")
            .json(&body)
    };

    // The same attempt in flight twice: both dispatch under the one
    // reservation — a retry is not a second spend.
    let first = send(call("shared-kev"));
    let second = send(call("shared-kev"));
    let (first, second) = tokio::join!(first.send(), second.send());
    assert_eq!(first.unwrap().status(), 200);
    assert_eq!(second.unwrap().status(), 200);
    assert_eq!(forwards.load(Ordering::SeqCst), 2);

    // Settled now: the same pair cannot be taken again, and a changed
    // body under it is refused rather than merged.
    let third = send(call("shared-kev")).send().await.unwrap();
    assert_eq!(third.status(), 409);
    let mut changed = call("shared-kev");
    changed["state"] = json!("Different text entirely.");
    let fourth = send(changed).send().await.unwrap();
    assert_eq!(fourth.status(), 409);
    assert_eq!(forwards.load(Ordering::SeqCst), 2);

    // A second gateway cannot open the same directory — the ledger
    // takes one writer.
    assert!(matches!(
        Ledger::open(deployment.dir.path()),
        Err(tenancy::quota::LedgerTrouble::Locked(_))
    ));
}

#[tokio::test]
async fn a_mismatched_backend_is_refused_before_dispatch() {
    // The backend publishes the wrong artifact for what the binding pins.
    let (endpoint, forwards) = backend(honest(artifact('z'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_mismatch");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);

    // The attempt is recorded unattempted and the reservation released —
    // the budget is not charged for work that never dispatched.
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unattempted);
}

#[tokio::test]
async fn a_dead_backend_is_unattempted_and_a_failed_forward_is_unavailable() {
    // Nothing listens on the endpoint — the identity check itself fails,
    // so the attempt is unattempted and uncharged.
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), "http://127.0.0.1:1".to_string())]
            .into_iter()
            .collect(),
    )
    .await;
    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "door_unavailable");
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unattempted);

    // A backend that answers but refuses with a 5xx is `unavailable` —
    // the attempt dispatched, and quota-v1 counts it.
    let (endpoint, _) = backend(Backend {
        answer_status: StatusCode::SERVICE_UNAVAILABLE,
        answer_body: json!({"error": {"code": "busy"}}),
        ..honest(artifact('a'), json!({}))
    })
    .await;
    let second = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;
    let response = send_call(&second, &call("shared-kev"), Some(&second.tokens["acme"])).await;
    assert_eq!(response.status(), 503);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "unavailable");
    let [receipt] = receipt_log(&second.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Unavailable);
    assert_eq!(receipt.cause.as_deref(), Some("unavailable"));
}

#[tokio::test]
async fn a_backend_refusal_passes_through_typed() {
    let (endpoint, _) = backend(Backend {
        answer_status: StatusCode::UNPROCESSABLE_ENTITY,
        answer_body: json!({"error": {"code": "too_many_options", "message": "…"}}),
        ..honest(artifact('a'), json!({}))
    })
    .await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 422);
    assert_eq!(response.headers().get("x-outcome").unwrap(), "refused");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "too_many_options");
    let [receipt] = receipt_log(&deployment.dir).try_into().unwrap();
    assert_eq!(receipt.outcome, receipts::execution::Outcome::Refused);
    assert_eq!(receipt.cause.as_deref(), Some("too_many_options"));
}

#[tokio::test]
async fn a_registry_update_names_its_revision_and_rebinds_the_next_call() {
    let (endpoint, _) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    // First call under sequence 0.
    send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;

    // The operator rebinds the door to artifact B — sequence 1. The
    // backend still publishes A, so the next call refuses as a mismatch
    // under the new revision, and each receipt names the revision its
    // call was admitted under.
    let mut updated = manifest(None);
    updated.sequence = 1;
    updated.supersedes = Some(
        Registry::open(deployment.dir.path())
            .unwrap()
            .digest()
            .to_string(),
    );
    updated
        .shared
        .get_mut("shared-kev")
        .unwrap()
        .artifact
        .artifact_signature = artifact('b');
    Registry::update(deployment.dir.path(), updated).unwrap();

    let response = send_call(
        &deployment,
        &call("shared-kev"),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 503);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_mismatch");

    let records = receipt_log(&deployment.dir);
    assert_eq!(records[0].registry.as_ref().unwrap().sequence, 0);
    assert_eq!(records[1].registry.as_ref().unwrap().sequence, 1);
    assert_ne!(
        records[0].registry.as_ref().unwrap().digest,
        records[1].registry.as_ref().unwrap().digest
    );
}

#[tokio::test]
async fn an_oversized_envelope_is_refused_before_a_door_is_consulted() {
    let (endpoint, forwards) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [("shared-kev".to_string(), endpoint)].into_iter().collect(),
    )
    .await;

    // 300 questions over a 256-question bound — refused at 422 with no
    // forward and no reservation taken.
    let mut questions = serde_json::Map::new();
    for index in 0..300 {
        questions.insert(
            format!("q{index}"),
            json!({"type": "noul", "instructions": "…", "criteria": "…"}),
        );
    }
    let response = send_call(
        &deployment,
        &json!({"model": "shared-kev", "state": "…", "questions": questions}),
        Some(&deployment.tokens["acme"]),
    )
    .await;
    assert_eq!(response.status(), 422);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "too_many_questions");
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn discovery_lists_the_callers_doors() {
    let (endpoint, _) = backend(honest(artifact('a'), json!({"answers": {}}))).await;
    let deployment = deploy(
        manifest(None),
        [
            ("shared-kev".to_string(), endpoint.clone()),
            ("acme-kev".to_string(), endpoint),
        ]
        .into_iter()
        .collect(),
    )
    .await;

    // acme sees its dedicated door plus the shared set.
    let body: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = body["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|card| card["id"].as_str())
        .collect();
    assert_eq!(ids, ["acme-kev", "shared-kev"]);

    // globex sees only the shared door — another tenant's dedicated
    // door is invisible, not merely unreachable.
    let body: Value = reqwest::Client::new()
        .get(format!("{}/v1/models", deployment.address))
        .bearer_auth(&deployment.tokens["globex"])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["models"].as_array().unwrap().len(), 1);
}

fn classify_call() -> Value {
    json!({
        "v":"openagents.classify.v1", "model":"acme-kev", "capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1", "name":"test-policy",
          "select":{"single_label":{"ties":"first-declared","no_match":{"kind":"null"}}}},
        "inputs":[{"id":"second","text":"one"},{"id":"first","text":"two"}],
        "mode":"single-label", "labels":[{"id":"a"},{"id":"b"}]
    })
}

#[tokio::test]
async fn classify_preserves_input_order_and_records_verified_native_answers() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({
        "model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}},
        "usage":{"input_tokens":3,"output_tokens":1}
    }))).await;
    let deployment = deploy_doors(
        manifest(None),
        [(
            "acme-kev".into(),
            Door {
                endpoint,
                classify: Some(gateway::classify::BackendLimits::product()),
            },
        )]
        .into_iter()
        .collect(),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(&classify_call())
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    assert_eq!(body["results"][0]["input"], "second");
    assert_eq!(body["results"][1]["input"], "first");
    assert_eq!(body["results"][0]["units"][0]["selected"], "a");
    assert_eq!(body["usage"]["input_tokens"], 6);
    assert_eq!(receipt_log(&deployment.dir).len(), 1);
}

#[tokio::test]
async fn classify_refuses_undeclared_limits_before_forwarding() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({}))).await;
    let deployment = deploy(
        manifest(None),
        [("acme-kev".into(), endpoint)].into_iter().collect(),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(&classify_call())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "unsupported_limits"
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

async fn classification_deployment(endpoint: String) -> Deployment {
    deploy_doors(
        manifest(None),
        [(
            "acme-kev".into(),
            Door {
                endpoint,
                classify: Some(gateway::classify::BackendLimits::product()),
            },
        )]
        .into_iter()
        .collect(),
    )
    .await
}

async fn send_classification(deployment: &Deployment, call: &Value) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .post(format!("{}/v1/classify", deployment.address))
        .bearer_auth(&deployment.tokens["acme"])
        .json(call)
        .send()
        .await
        .unwrap();
    (response.status(), response.json().await.unwrap())
}

#[tokio::test]
async fn classify_retains_partial_outcomes_and_does_not_invent_complete_usage() {
    let forwards = Arc::new(AtomicUsize::new(0));
    let counter = forwards.clone();
    let app = axum::Router::new()
        .route("/v1/models", get(|| async { Json(json!({"models":[{"id":"kev-0.6b", "artifact_identity":{"digest":artifact('b')}, "execution":{}}]})) }))
        .route("/v1/systemone", post(move || {
            let index = counter.fetch_add(1,Ordering::SeqCst);
            async move {
                if index == 0 { (StatusCode::OK,Json(json!({"model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}}, "usage":{"input_tokens":3,"output_tokens":1}}))) }
                else { (StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"unavailable"}))) }
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    let deployment = classification_deployment(endpoint).await;
    let mut call = classify_call();
    call["inputs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"third","text":"three"}));
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "mixed");
    assert_eq!(
        body["outcomes"],
        json!({"answered":1,"unavailable":1,"unattempted":1,"refused":0})
    );
    assert_eq!(forwards.load(Ordering::SeqCst), 2);
    assert_eq!(body["results"][2]["input"], "third");
    assert_eq!(body["usage"]["input_tokens_complete"], false);
    assert!(body["usage"].get("input_tokens").is_none());
    assert_eq!(body["results"][0]["usage"]["input_tokens"], 3);
    let ledger = std::fs::read_to_string(deployment.dir.path().join("quota-ledger.jsonl")).unwrap();
    let settled: Value = ledger
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|event| event["event"] == "settled")
        .expect("the attempt settled");
    assert_eq!(settled["units"]["questions"], 2);
    assert_eq!(settled["units"]["options"], 4);
}

#[tokio::test]
async fn classify_rejects_wrong_models_and_invalid_distributions() {
    for answer in [
        json!({"model":"other", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}}}),
        json!({"model":"kev-0.6b", "answers":{"q0":{"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.8}}}}),
    ] {
        let (endpoint, _) = backend(honest(artifact('b'), answer)).await;
        let deployment = classification_deployment(endpoint).await;
        let (status, body) = send_classification(&deployment, &classify_call()).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["outcomes"]["answered"], 0);
        assert!(body["results"][0]["units"][0]["selected"].is_null());
    }
}

#[tokio::test]
async fn an_unfinished_chunked_response_is_refused_at_the_byte_limit() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buffer[..n]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 16384);
            }
            if index == 0 {
                assert!(request.starts_with(b"GET /v1/models"));
                let body=json!({"models":[{"id":"kev-0.6b","artifact_identity":{"digest":artifact('b')},"execution":{}}]}).to_string();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(body.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            } else {
                assert!(request.starts_with(b"POST /v1/systemone"));
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n400001\r\n")
                    .await
                    .unwrap();
                // Deliberately never send the terminating chunk or close the
                // connection. A read-to-end implementation waits for timeout.
                let bytes = vec![b'x'; 4_194_305];
                let _ = socket.write_all(&bytes).await;
                std::future::pending::<()>().await;
            }
        }
    });
    let deployment = classification_deployment(endpoint).await;
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        send_classification(&deployment, &classify_call()),
    )
    .await;
    server.abort();
    let (status, body) =
        result.expect("the byte bound refuses without waiting for response completion");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body["results"][0]["cause"]
            .as_str()
            .unwrap()
            .contains("exceeded 4194304 bytes"),
        "{body}"
    );
    assert_eq!(body["results"][1]["outcome"], "unattempted");
}

/// A stub that answers each forward from the request's state — the seam
/// the per-input modes need.
fn per_input_backend(
    respond: impl Fn(&Value) -> (StatusCode, Value) + Send + Sync + 'static,
) -> Backend {
    Backend {
        respond: Some(Arc::new(respond)),
        ..honest(artifact('b'), json!({}))
    }
}

#[tokio::test]
async fn classify_binary_sends_one_noul_and_selects_the_declared_subset() {
    // The noul rises with the input's own text: "keep" is over the
    // caller's 0.5 cut, "edge" lands on it, "drop" falls under.
    let stub = per_input_backend(|body| {
        let state = body["state"].as_str().unwrap_or_default();
        let noul = if state.contains("keep") {
            0.9
        } else if state.contains("edge") {
            0.5
        } else {
            0.1
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"noul","noul":noul}},
                   "usage":{"input_tokens":3,"output_tokens":1}}),
        )
    });
    let bodies = stub.bodies.clone();
    let (endpoint, forwards) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"filter",
          "select":{"binary":{"threshold":0.5}}},
        "inputs":[{"id":"a","text":"keep this"},{"id":"b","text":"drop this"},{"id":"c","text":"edge case"}],
        "mode":"binary","labels":[{"id":"keep","description":"Worth keeping"}]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "answered");
    assert_eq!(forwards.load(Ordering::SeqCst), 3);

    // Each forward asked exactly one noul question — one judgment per
    // input, the filter's label its only criterion.
    for sent in bodies.lock().unwrap().iter() {
        let questions = sent["questions"].as_object().unwrap();
        assert_eq!(questions.len(), 1);
        assert_eq!(questions["q0"]["type"], "noul");
        assert!(
            questions["q0"]["instructions"]
                .as_str()
                .unwrap()
                .contains("Worth keeping")
        );
    }

    // Per-input outcomes keep input order; the corpus subset names the
    // ids the threshold admitted — the boundary case included.
    assert_eq!(body["results"][0]["input"], "a");
    assert_eq!(body["results"][1]["input"], "b");
    assert_eq!(body["results"][2]["input"], "c");
    assert_eq!(body["results"][0]["units"][0]["selected"], "keep");
    assert!(body["results"][1]["units"][0]["selected"].is_null());
    assert_eq!(body["results"][2]["units"][0]["selected"], "keep");
    assert_eq!(body["results"][0]["units"][0]["raw"]["noul"], 0.9);
    assert_eq!(
        body["selections"],
        json!([{"mode":"binary","label":"keep","selected":["a","c"],"unevaluated":[]}])
    );
    assert_eq!(receipt_log(&deployment.dir).len(), 1);
}

#[tokio::test]
async fn classify_score_ranks_inputs_on_the_declared_rubric() {
    // Three inputs, three positions on one rubric: a scores highest,
    // then c, then b — the ranking is the call's, not input order.
    let rubric = json!({"0":"weak","1":"fair","2":"strong"});
    let rubric_for_answer = rubric.clone();
    let stub = per_input_backend(move |body| {
        let (score, level, probabilities) = match body["state"].as_str().unwrap_or_default() {
            "a-input" => (2.0, "2", json!({"0":0.0,"1":0.0,"2":1.0})),
            "b-input" => (0.4, "0", json!({"0":0.8,"1":0.2,"2":0.0})),
            _ => (1.2, "1", json!({"0":0.2,"1":0.6,"2":0.2})),
        };
        (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"score","score":score,"confidence":0.9,
                                   "legend":rubric_for_answer,"selected":level,
                                   "probabilities":probabilities}},
                   "usage":{"input_tokens":4,"output_tokens":2}}),
        )
    });
    let bodies = stub.bodies.clone();
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending"}}},
        "inputs":[{"id":"a","text":"a-input"},{"id":"b","text":"b-input"},{"id":"c","text":"c-input"}],
        "mode":"score","levels":["weak","fair","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The forward carried the rubric as an ordered score criteria.
    let questions = bodies.lock().unwrap()[0]["questions"].clone();
    assert_eq!(questions["q0"]["type"], "score");
    assert_eq!(
        questions["q0"]["criteria"],
        json!(["weak", "fair", "strong"])
    );

    // Raw positions survive; `selected` is the categorical level the
    // estimator named; the corpus ranking orders by weighted position.
    assert_eq!(body["results"][0]["units"][0]["raw"]["score"], 2.0);
    assert_eq!(body["results"][0]["units"][0]["selected"], 2);
    assert_eq!(body["results"][2]["units"][0]["raw"]["score"], 1.2);
    assert_eq!(
        body["selections"],
        json!([{"mode":"score","ranking":["a","c","b"],"unevaluated":[]}])
    );
}

#[tokio::test]
async fn classify_refuses_invalid_rubrics_and_undeclared_rules() {
    let (endpoint, forwards) = backend(honest(artifact('b'), json!({}))).await;
    let deployment = classification_deployment(endpoint).await;
    let base = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"descending"},"binary":{"threshold":0.5}}},
        "inputs":[{"id":"a","text":"x"}],
        "mode":"score","levels":["weak","strong"]
    });

    // A one-level rubric, an over-maximum rubric, a rubric beside a
    // label set, and a binary unit with two labels all refuse before
    // any forward.
    let mut under_minimum = base.clone();
    under_minimum["levels"] = json!(["only"]);
    let mut over_maximum = base.clone();
    over_maximum["levels"] = json!((0..=10).map(|n| format!("level {n}")).collect::<Vec<_>>());
    let mut with_labels = base.clone();
    with_labels["labels"] = json!([{"id":"x"}]);
    let mut wide_binary = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":base["policy"].clone(),
        "inputs":[{"id":"a","text":"x"}],
        "mode":"binary","labels":[{"id":"keep"},{"id":"drop"}]
    });
    for call in [
        under_minimum,
        over_maximum,
        with_labels,
        wide_binary.clone(),
    ] {
        let (status, body) = send_classification(&deployment, &call).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    // Eleven levels is a count refusal, the rest invalid envelopes.
    let mut levels_11 = base.clone();
    levels_11["levels"] = json!((0..11).map(|n| format!("level {n}")).collect::<Vec<_>>());
    let (_, body) = send_classification(&deployment, &levels_11).await;
    assert_eq!(body["error"]["code"], "too_many_levels");

    // A mode the policy does not declare is refused rather than
    // assigned a rule.
    let mut undeclared = base.clone();
    undeclared["policy"]["select"]["score"] = Value::Null;
    let (status, body) = send_classification(&deployment, &undeclared).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_request");
    wide_binary["policy"]["select"]["binary"] = Value::Null;
    let (status, _) = send_classification(&deployment, &wide_binary).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    assert_eq!(forwards.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn classify_score_keeps_outcome_order_when_forwards_fail() {
    // One input answers, one is refused by the backend, one is
    // unavailable: the corpus ranking names only the scored input and
    // `unevaluated` carries the rest rather than dropping them.
    let stub = per_input_backend(|body| match body["state"].as_str().unwrap_or_default() {
        "a-input" => (
            StatusCode::OK,
            json!({"model":"kev-0.6b",
                   "answers":{"q0":{"type":"score","score":0.75,"confidence":0.9,
                                   "legend":{"0":"weak","1":"strong"},
                                   "probabilities":{"0":0.25,"1":0.75}}}}),
        ),
        "b-input" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error":{"code":"too_many_options","message":"…"}}),
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":{"code":"busy"}}),
        ),
    });
    let (endpoint, _) = backend(stub).await;
    let deployment = classification_deployment(endpoint).await;
    let call = json!({
        "v":"openagents.classify.v1","model":"acme-kev","capacity":"dedicated",
        "policy":{"v":"openagents.classify-policy.v1","name":"rubric",
          "select":{"score":{"order":"ascending"}}},
        "inputs":[{"id":"a","text":"a-input"},{"id":"b","text":"b-input"},{"id":"c","text":"c-input"}],
        "mode":"score","levels":["weak","strong"]
    });
    let (status, body) = send_classification(&deployment, &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "mixed");
    assert_eq!(
        body["outcomes"],
        json!({"answered":1,"refused":1,"unavailable":1,"unattempted":0})
    );
    assert_eq!(body["results"][0]["input"], "a");
    assert_eq!(body["results"][1]["input"], "b");
    assert_eq!(body["results"][1]["outcome"], "refused");
    assert_eq!(body["results"][1]["cause"], "too_many_options");
    assert_eq!(body["results"][2]["input"], "c");
    assert_eq!(body["results"][2]["outcome"], "unavailable");
    assert_eq!(
        body["selections"],
        json!([{"mode":"score","ranking":["a"],"unevaluated":["b","c"]}])
    );
}
