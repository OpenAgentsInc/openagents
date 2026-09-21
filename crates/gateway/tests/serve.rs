//! The gateway's end-to-end contract: real HTTP both ways, a stubbed
//! backend publishing a model card, and a real registry, key store, and
//! ledger on disk. Each test stands up its own directory and listeners —
//! nothing shares state but the shape of the claims being checked.

use std::collections::BTreeMap;
use std::sync::Arc;
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

async fn backend_systemone(State(backend): State<Arc<Backend>>, _body: Bytes) -> Response {
    backend.forwards.fetch_add(1, Ordering::SeqCst);
    if backend.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(backend.delay_ms)).await;
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
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let doors = endpoints
        .into_iter()
        .map(|(door, endpoint)| (door, Door { endpoint }))
        .collect();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
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
